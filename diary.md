# Diary

> The diary details interesting parts of building this asynchronous runtime.

This code is taken from another crate of mine, [`woi`](https://github.com/senyosimpson/woi) which is
a single-threaded runtime built for Linux. It contains a [diary](https://github.com/senyosimpson/woi/blob/main/diary.md)
journaling its development. It's a good place to start reading. This diary will continue from there.

## 21 March 2022

Story time! The main feature I've been working towards is `no_std` compatibility of my runtime, in order
to get it to run on embedded. This comes with a number of challenges:

1. No heap allocations
2. No thread local storage
3. No `epoll` (or similar) system
4. No stack unwinding panics
5. No network stack

We'll go over the work I've completed so far.

### Thread local storage

Embedded systems do not have thread local storage capabilities. The runtime uses it in `runtime/context.rs`
to store variables globally. Some sorcery was used to rewrite the thread local storage code without it.
The main ingredient in making this work was using creating a `Context` type. It has functions that allow
it to be used as a constant variable, allowing us access it to globally.
via a `const fn`.

```rust

#[derive(Clone)]
pub(crate) struct Context(RefCell<Handle>>);

impl Context {
    const fn new() -> Context {
        Context(RefCell::new(None))
    }
}
```

We can now store a static variable

```rust
// This requires `new()` to be a constant function
static CONTEXT: Context = Context::new();
```

This gives us all the functionality we need for our purposes. We can set the various handles to our
runtime. Most of the code remains the same, with minor tweaks here and there.

### No heap allocations

This was quite a battle. In its current version, it's pretty ugly lmao. I have some ideas to fix it
but those can come later. Let's get into it!

The first part is that I'm using `VecDeque` to store tasks in the runtime. Since we don't have a heap,
we need to statically allocate memory. This presents a few problems:

1. How do we even do this?
2. How do we choose an upper bound for the size of the new collection?

The first is really simple - use [`heapless`](https://docs.rs/heapless/latest/heapless/)!. It has a number
of collections that can be statically allocated. Of interest here is `heapless::Deque`. This has the
same (or similar) functionality and API as `VecDeque`, making it a drop-in replacement. The only problem
is that since it is statically allocated, there has to be a maximum size. In reality, choosing this is
non-trivial and depends entirely on the runtime characteristics of your application. For my use case,
I have just chosen an arbitrary number - 1024. If we try to run more than 1024 tasks, the tasks will
be lost.

The second part was removing allocating futures on the heap. This required quite extensive changes
to the internals of `RawTask`. I think I could refine it but bear with me for now. First, `RawTask`
needs somewhere to store data.

```rust
// This holds our data and is statically allocated
struct Memory<F, S> {
    /// Header of the task. Contains data related to the state
    /// of a task
    pub header: UninitCell<Header>,
    /// Scheduler is responsible for scheduling tasks onto the
    /// runtime. When a task is woken, it calls the related
    /// scheduler to schedule itself
    pub scheduler: UninitCell<S>,
    /// The status of a task. This is either a future or the
    /// output of a future
    pub status: UninitCell<Status<F>>,
}
```

Importantly, `Memory` has a function `const fn alloc()`

```rust
impl<F, S> Memory<F, S>
where
    F: Future,
    S: Schedule,
{
    pub const fn alloc() -> Self {
        Memory {
            header: UninitCell::uninit(),
            scheduler: UninitCell::uninit(),
            status: UninitCell::uninit(),
        }
    }
}
```

`UninitCell` is a custom implementation found in `task/cell.rs`. It is copied as is from `embassy`.
The reason we use it is that we first need to allocate the memory, then we can populate it with data.

Our `RawTask` looks like

```rust
pub struct RawTask<F: Future, S> {
    // pub ptr: *const Memory<F, S>,
    pub ptr: *mut (),
    pub(crate) _f: PhantomData<F>,
    pub(crate) _s: PhantomData<S>,
}
```

where `ptr` points to the memory we statically allocate.

```rust
impl<F, S> RawTask<F, S>
where
    F: Future,
    S: Schedule,
{
    pub fn new(memory: &Memory<F, S>, future: impl FnOnce() -> F) -> RawTask<F, S> {
        let id = TaskId::new();

        let header = Header {
            id,
            state: State::new_with_id(id),
            waker: None,
            vtable: &TaskVTable {
                poll: Self::poll,
                get_output: Self::get_output,
                drop_join_handle: Self::drop_join_handle,
            },
        };

        // NOTE: The scheduler is written when a task is spawned

        unsafe { memory.header.write(header) }

        let status = Status::Running(future());
        unsafe { memory.status.write(status) };

        let ptr = memory as *const _ as *mut ();
        RawTask {
            ptr,
            _f: PhantomData,
            _s: PhantomData,
        }
    }
}
```

Now the important question: how do we actually allocate this memory? The way `embassy` has done it is
through a proc macro. This is probably the best way since it saves a ton of boilerplate.

```rust
#[embassy::task]
async fn doing_async_stuff() {
    // Do some async stuff
}
```

Getting this to work requires nightly features (`type_alias_impl_trait`). Our macro can be found at
`chrono-macros/alloc.rs`. The gist of it comes down to this part

```rust
let fn_name = f.sig.ident.clone();
let fn_args = f.sig.inputs.clone();
let visibility = &f.vis;
let attrs = &f.attrs;

f.sig.ident = format_ident!("task");

let impl_ty = quote! (impl ::core::future::Future);

quote! {
    #(#attrs)*
    #visibility fn #fn_name(#fn_args) -> ::chrono::task::RawTask<#impl_ty, ::chrono::runtime::Queue> {
        use ::chrono::task::Memory;
        #f

        // `type_alias_impl_trait` is necessary here. Self-explanatory no?
        type F = #impl_ty;

        static MEMORY: Memory<F, Queue> = Memory::alloc();
        RawTask::new(&MEMORY, move || task(#arg_names))
    }
}.into()
```

All this really does is rewrite the async function so it looks something like

```rust
fn doing_async_stuff() -> RawTask<impl Future, Queue> {
    use chrono::task::Memory

    // The name of the original function is changed to task() and the outer
    // function takes the name of the original function
    async fn task(a: u32, b: u16) {
        // Do some async stuff
    }

    type F = impl Future;

    // Here is where our memory is allocated
    static MEMORY: Memory<F, Queue> = Memeory::alloc();
    RawTask::new(&MEMORY, move || task(a: u32, b: u16))
}
```

The static declaration allocates our memory. Then we pass it into `RawTask::new()` which writes
the future and task header into the allocated memory and returns a `RawTask`. We spawn the `RawTask`
onto the runtime. This is fairly different from before where we would spawn the future directly. That's
not possible here since we need to allocate the memory upfront. A main function would then look like

```rust
#[chrono::alloc]
async fn doing_async_stuff() {

}

fn main() {
    let rt = Runtime::new();
    rt.block_on(async {
        rt.spawn(doing_async_stuff())
    })
}
```

An interesting change from before as well is that `RawTask::new` takes in a function that produces a
future instead of a future itself. I tried the standard way of passing in a future directly but it
did not work due to an error `could not find defining uses`. Googling that only gave me discussions that
are too advanced for my understanding of programming languages. However, changing it fixed this for me
and so that's why it is done as such.
