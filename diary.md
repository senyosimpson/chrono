# Diary

> The diary details interesting parts of building this asynchronous runtime.

The code is adapted from another crate of mine, [`woi`](https://github.com/senyosimpson/woi) -- a
single-threaded runtime built for Linux. It contains a [diary](https://github.com/senyosimpson/woi/blob/main/diary.md)
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
// This requires `new()` to be a const function
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

The second part was removing allocating futures on the heap. This required extensive changes to the
internals of `RawTask`. It's ugly but bear with me for now. First, `RawTask` needs somewhere to store
data.

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

## 6 November 2022

Can't believe how much time has passed ha. And with that, many many coding changes. Let's walk through
them.

### Refactoring `RawTask`

In the initial design, a `RawTask` was defined as:

```rust
pub struct RawTask<F: Future, S> {
    pub ptr: *mut (),
    pub(crate) _f: PhantomData<F>,
    pub(crate) _s: PhantomData<S>,
}
```

For some or other reason, type inference was not working well and the output type for a `JoinHandle`
would be expressed as a `Future` with generic parameters. The type was impossible to reason about.
To fix it, `RawTask` was changed to

```rust
pub struct RawTask<F: Future<Output = T>, T, S> {
    pub ptr: *mut (),
    pub(crate) _f: PhantomData<F>,
    pub(crate) _s: PhantomData<S>,
}
```

However, doing this created problems with the `alloc` macro. The generic parameter `T` could not be
automatically inferred for the output type. We can solve this simply

```rust
let fn_ret = {
    match f.sig.output.clone() {
        ReturnType::Default => {
            let ret = ReturnType::Default;
            quote!(::chrono::task::RawTask<#impl_ty, #ret, ::chrono::runtime::Queue>)
        },
        ReturnType::Type(_, ret) => {
            quote!(::chrono::task::RawTask<#impl_ty, #ret, ::chrono::runtime::Queue>)
        }
    }
};

quote! {
    #(#attrs)*
    // we use #fn_ret as the return type
    #visibility fn #fn_name(#fn_args) -> #fn_ret {
        use ::chrono::task::Memory;
        #f

        type F = #impl_ty;

        static MEMORY: Memory<F, Queue> = Memory::alloc();
        RawTask::new(&MEMORY, move || task(#arg_names))
    }
}.into()
```

### Death to a maximum number of tasks

Okay, only kind of. To make `chrono` `no_std` compatible, `VecDeque` was replaced with a
non-allocating version from the `heapless` crate. Since it is statically allocated, we
have to choose a size for our queue, effectively placing a limit on the number of tasks
that can be spawned simultaneously. We can't remove this restriction entirely but we can
improve it.

In `embassy`, intrusive linked lists are used to queue tasks. We adopt the same strategy. Each
task contains pointers to the previous and next task in the queue.

```rust
pub struct Task {
    pub raw: NonNull<()>,
    pub(crate) tasks: Pointers,
}

pub(crate) struct Pointers {
    next: Option<NonNull<Task>>,
    prev: Option<NonNull<Task>>,
}
```

Since we use an intrusive data structure, the queue automagically has the capacity we need to run
every task we allocate. We're only limited by the number of tasks we can fit in memory.

This design breaks down slightly if we want to spawn `n` of the *same* task. We still have to
decide how many of that task we need to meet our requirements. This may be non-trivial, for example,
if we need to accept TCP connections concurrently. How many "TCP process connection" tasks do we need?
Depends on your use case. However, once allocated, we know that we have capacity for those `n` tasks
and for every other tasks. This is much nicer since we don't have a capacity limit applied globally
(i.e spawn only `n` tasks in total).

### A journey into runtime design, intrusive data structures & static allocations

Above I mentioned using an intrusive linked list for our task queue. Since we're using our own
data structure, we have two challenges:

  1. We have to implement it ourselves
  1. We have to figure out how to statically allocate it
  
Let's talk about the linked list. It's in `runtime/queue.rs`. The linked list is defined as

```rust
pub(crate) struct LinkedList {
    pub head: Cell<Option<NonNull<Task>>>
    pub tail: Cell<Option<NonNull<Task>>>
}

// The runtime processes the tasks in a FIFO manner (though its quite common
// to do LIFO for better locality). Hence we push onto the back of the queue
// and pop from the front
impl LinkedList {
    pub fn push_back(&mut self, task: NonNull<Task>) {
        // If there are 1 or more tasks in the queue:
        //   1. Get the tail task and
        //   2. Set its "next" pointer to the input task
        //   3. Replace the tail with the input task
        if let Some(mut tail) = self.tail.get() {
            unsafe { tail.as_mut().tasks.set_next(Some(task)) };
            self.tail.replace(Some(task));
            return;
        }

        // If there are no elements in the queue, set the input task to both
        // the head and the tail
        self.head.replace(Some(task));
        self.tail.replace(Some(task));
    }

    pub fn pop_front(&self) -> Option<&mut Task> {
        // If there is a task at the head of the queue:
        //   1. Check if it has a "next" pointer. If it's null, we know we're
        //      the last element in the queue and can set the head and tail to
        //      null and return the task
        //   2. If not, we set the head to the "next" pointer of the current head
        //      task and return the current task
        match self.head.get() {
            None => None,
            Some(mut head) => {
                let curr = unsafe { head.as_mut() };
                if curr.tasks.next().is_none() {
                    // We are on the last element in the queue. Set
                    // head and tail to None and return the task
                    self.head.replace(None);
                    self.tail.replace(None);
                    return Some(curr);
                }

                // Set the head to the next task the current head
                // is pointing to
                self.head.replace(curr.tasks.next());
                // Set next timer in the current task to null
                curr.tasks.set_next(None);
                // Return the current task
                Some(curr)
            }
        }
    }
}
```

We can now change `Runtime` to use our data structure.

```rust
use heapless::Arc;

pub struct Runtime {
    queue: Arc<LinkedList>,
    handle: Handle
}

struct Spawner {
    queue: Arc<LinkedList>
}
```

Both `Runtime` and `Spawner` refer to the same `LinkedList`. Initially, we used [`heapless::Arc`](https://docs.rs/heapless/latest/heapless/pool/singleton/arc/struct.Arc.html)
to be able to share the `LinkedList`. However, as one would imagine, we need to choose a size to
statically allocate the backing memory for `Arc`. Since every `RawTask` also needs to keep a reference
to the queue. Our backing memory then has to accommodate the maximum number of tasks we will ever spawn.
We're back at limiting our maximum number of tasks - no good. Not to fear! We can get around this using
some (unsafe) Rust. Our idea is simple: create a queue and store a pointer to that queue in `Spawner`
and the tasks directly. That way, everything is allocated at compile time.

```rust
pub struct Runtime {
    queue: LinkedList,
    handle: Handle
}

struct Spawner {
    queue: *mut LinkedList
}

impl Runtime {
    // Simplified for the purposes of the explanation
    pub fn new() -> Runtime {
        // Create a queue
        let queue = LinkedList::new();
        // Get a pointer to the queue for Spawner so we can spawn
        // tasks onto the runtime.
        let queue_ptr = unsafe { &queue as *const _ as *mut LinkedList };
        let handle = Handle { spawner: Spawner::new(queue_ptr) };

        Runtime { queue, handle }
    }

    pub fn spawn<F: Future<Output = T>, T>(
        &self,
        raw: RawTask<F, T>,
    ) -> Result<JoinHandle<T>, SpawnError> {
        self.handle().spawn(raw)
    }
}
```

If you are a much better systems/embedded programmer than I am, you'll realise that this *does not* work.
And oh my days did it take me some time to figure this out. Here's why:

```rust
impl Runtime {
    pub fn new() -> Runtime {
        // Imagine queue is stored at address 0x12345
        let queue = LinkedList::new();
        // We grab a pointer that points to 0x12345
        let queue_ptr = unsafe { &queue as *const _ as *mut LinkedList };
        let handle = Handle { spawner: Spawner::new(queue_ptr) };

        // Here, queue is *moved* into Runtime which is at some other address,
        // 0x1234imanidiot
        Runtime { queue, handle }
}
```

Now, when you try and spawn a task onto the runtime, `queue_ptr` points to an invalid address. How wonderful.
So our problem is that `queue` moves after we take a pointer to it. How do we ensure it has a fixed address?
Yessir, a `static` variable. One way you could do this (which I initially had)

```rust
pub struct Runtime {
    queue: *mut LinkedList,
    handle: Handle
}

struct Spawner {
    queue: *mut LinkedList
}

impl Runtime {
    pub fn new() -> Runtime {
        // static var for queue so that it has a fixed memory address
        static mut queue: LinkedList = LinkedList::new();
        let queue_ptr = unsafe { &queue as *const _ as *mut Queue };

        let spawner = Spawner { queue: queue_ptr };
        let handle = Handle { spawner };

        Runtime { queue: queue_ptr, handle }
    }
}
```

Now we have something that works. I personally didn't like the design though. I felt like I could
have something that reads better. In `embassy`, they statically allocate the executor (in our case, `Runtime`)
and each task stores a pointer to the runtime. We apply the same design.

```rust
pub struct Memory<F: Future<Output = T>, T> {
    /// Header of the task. Contains data related to the state
    /// of a task
    pub header: UninitCell<Header>,
    /// Pointer to the runtime
    pub(crate) rt: Cell<NonNull<Runtime>>,
    /// The status of a task. This is either a future or the
    /// output of a future
    pub status: UninitCell<Status<F, T>>,
}
```

> Notice how we don't have store a `Scheduler` any longer.

As you can see, we have a pointer to the runtime stored in `Memory`. When we spawn a task, we update
the pointer to the address of the runtime. This is how we'd use `chrono` now

```rust
#[chrono::alloc]
async fn doing_async_stuff() {

}

fn main() {
    static mut rt: Runtime = Runtime::new();
    rt.block_on(async {
        rt.spawn(doing_async_stuff())
    })
}
```

This reads much better even though we now have a mutable static. Luckily for us, we're only operating
in a single-threaded context so we can use it without worry.

### Implementing timers

Timers was fun to implement but probably the most difficult part given that I knew (know?) nothing
about timers.

First off, we need to interface with the hardware timers. Fortunately, Rust has many crates for
interfacing with embedded systems. I used the `stm32f3xx_hal` [crate](https://docs.rs/stm32f3xx-hal/latest/stm32f3xx_hal/index.html).
The crate has an [example](https://github.com/stm32-rs/stm32f3xx-hal/blob/v0.9.1/examples/adc.rs) of
how to use the timers. I shamelessly copied it with the only real difference being how it is structured
in the code.

### `no_std` channels

Channels had to be retrofitted to suit a `no_std` context. Two main things:

  1. I had only implemented an unbounded channel with `woi`. Obviously, this won't work in this context,
     it has to be bounded.
  1. We need some way of sharing the channel between the `Sender` and `Receiver`. Previously we used
     `Rc` but we don't have access to that in `no_std` land.

Solving the bounded case is pretty straightforward. The `Channel` for `std` used a `VecDeque` internally.
We replace that with `Deque` from the `heapless` crate. It requires us to set a capacity on initialisation.
When the queue is full, it throws an error. In that way, we have a bounded queue without the need for
primitives like semaphores.

```rust
// Simplified for the sake of example

pub struct Channel<T, const N: usize> {
    /// Queue holding messages
    queue: Deque<T, N>,
    /// Number of outstanding sender handles. When it drops to
    /// zero, we close the sending half of the channel
    tx_count: usize,
    /// State of the channel
    state: State,
    /// Waker notified when items are pushed into the channel
    rx_waker: Option<Waker>,
}

enum State {
    Open,
    Closed,
}

// ===== impl Channel =====

impl<T, const N: usize> Channel<T, N> {
    pub const fn new() -> Channel<T, N> {
        Channel {
            queue: Deque::new(),
            tx_count: 1,
            state: State::Open,
            rx_waker: None,
        }
    }
}
```

We use const generics to parameterise the input size of the channel. I thought they were much more
complicated to use but turns out, pretty simple.

Sharing a channel between the `Sender` and `Receiver` turned out to be simple but required a redesign.
As I said, initially we used an `Rc` to share them but in `no_std` land, we don't have that luxury.
I wanted to keep the same API as `no_std` when creating a channel.

```rust
let (tx, rx) = mspc::unbounded::channel();
```

However, without `Rc`, you're left with either holding a pointer to an already allocated channel or
a reference to the channel. Neither of those fit the above API. So, taking inspiration from `embassy`,
I decided to share a reference to the channel between the two halves.

```rust
pub struct Sender<'a, T, const N: usize> {
    chan: &'a Channel<T, N>,
}

pub struct Receiver<'a, T, const N: usize> {
    chan: &'a Channel<T, N>,
}

pub const fn channel<T, const N: usize>() -> Channel<T, N> {
    Channel::new()
}


// Splits a channel into Sender and Receiver halves. Each half
// contains a reference to the channel. 
pub fn split<T, const N: usize>(chan: &Channel<T, N>) -> (Sender<T, N>, Receiver<T, N>) {
    (Sender { chan }, Receiver { chan })
}
```

We can use that in code like so:

```rust
#[chrono::alloc]
async fn send(tx: Sender<'static, &str, 5>) {
    tx.send("hello world").unwrap();
}

#[chrono::alloc]
async fn receive(rx: Receiver<'static, &str, 5>) {
    defmt::info!("Received message: {}", rx.recv().await.unwrap());
}

#[chrono::main]
async fn main() -> ! {
        // Create a static for the channel so it is guaranteed to
        // last the lifetime of the program
        static CHANNEL: Channel<&str, 5> = mpsc::channel();

        // Now we split it into sender and receiver halves
        let (tx, rx) = mpsc::split(&CHANNEL);
        
        let res = chrono::spawn(send(tx));
        let handle = match res {
            Ok(handle) => handle,
            Err(_) => panic!("Could not spawn task!"),
        };
        let _output = handle.await;

        let res = chrono::spawn(receive(rx));
        let handle = match res {
            Ok(handle) => handle,
            Err(_) => panic!("Could not spawn task!"),
        };
        let _output = handle.await;

    defmt::info!("Success!");
}
```

### Building a net stack

I'm still in the process of building the net stack but most of the work has been done. It only
supports ethernet via an ENC28J60 breakout board. Why this board? Because [japaric](https://twitter.com/japaric_io)
wrote a [driver](https://docs.rs/enc28j60/) for it. There's a [blog post](https://blog.japaric.io/wd-4-enc28j60/)
about it.

Doing networking on embedded means we need a TCP/IP stack. The most common one is [lwIP](https://savannah.nongnu.org/projects/lwip/)
but its written in C. Fortunately, there's [smoltcp](https://docs.rs/smoltcp/latest/smoltcp/) which
is written in Rust as a replacement for lwIP.

To start off, we need to setup and initialise the ENC28J60 device. There's an example [here](https://github.com/japaric/stm32f103xx-hal/blob/ed402cfaf09c5d0723fb2e751173a6aab3bca8ff/examples/enc28j60.rs#L50-L104)
of how to do it. I just copied it, only making changes where there are differences in the embedded boards.

For our device to work with `smoltcp`, we have to implement the [`Device`](https://docs.rs/smoltcp/latest/smoltcp/phy/trait.Device.html)
trait. The main functions are `receive` and `transmit`. These produce tokens which are "types that allow
to receive/transmit a *single* packet". We have to implement the tokens ourselves.

```rust
// Max transmission unit
const MTU: usize = 1514;

pub struct RxToken {
    // the buffer to hold a packet, bounded by the MTU
    buffer: [u8; MTU], 
    // the actual size of the packet (as it may be less than the MTU)
    size: u16, 
}

// Implement the RxToken trait for our RxToken
impl phy::RxToken for RxToken {
    // This method is called internally by smoltcp to receive a packet. It applies
    // some function `f` to the data in the packet. In our case, we let the `receive`
    // function in the `Device` trait do all the work of receiving data from the device
    fn consume<R, F>(mut self, _: smoltcp::time::Instant, f: F) -> smoltcp::Result<R>
    where
        F: FnOnce(&mut [u8]) -> smoltcp::Result<R>,
    {
        f(&mut self.buffer[..self.size as usize])
    }
}

pub struct TxToken<'a, T: Device<'a>> {
    // Hold a reference to the device so we can transmit
    // packets from it
    device: &'a mut T,
    phantom: PhantomData<&'a T>,
}

// Implement the TxToken trait for our TxToken
impl<'a> phy::TxToken for TxToken<'a, Enc28j60> {
    fn consume<R, F>(self, _: smoltcp::time::Instant, len: usize, f: F) -> smoltcp::Result<R>
    where
        F: FnOnce(&mut [u8]) -> smoltcp::Result<R>,
    {
        // Create a buffer with max size MTU
        let mut buffer = [0; MTU];
        // We only need to send up to len bytes, hence we slice it
        let packet = &mut buffer[..len];
        let result = f(packet);
        // This actually sends the packets from the device
        self.device
            .transmit(packet)
            .expect("Could not transmit packets");

        result
    }
}
```

Now that we have that, all we have to do is implement `Device`

```rust
impl<'a> Device<'a> for Enc28j60 {
    type RxToken = RxToken;

    type TxToken = TxToken<'a, Enc28j60>;

    fn capabilities(&self) -> DeviceCapabilities {
        let mut dc = DeviceCapabilities::default();
        dc.medium = Medium::Ethernet;
        dc.max_transmission_unit = MTU;
        dc.max_burst_size = Some(0);

        dc
    }

    // As mentioned earlier, the receive function does all the work in receiving
    // packets from the device, leaving RxToken only to apply the function to it
    fn receive(&'a mut self) -> Option<(Self::RxToken, Self::TxToken)> {
        match self.pending_packets() {
            Err(_) => panic!("failed to check if pending packets"),
            Ok(n) if n == 0 => None,
            Ok(_) => {
                let mut buffer = [0; MTU];
                match self.receive(&mut buffer) {
                    Ok(size) => {
                        let rx = RxToken { buffer, size };
                        let tx = TxToken {
                            device: self,
                            phantom: PhantomData,
                        };
                        Some((rx, tx))
                    },
                    Err(_) => panic!("failed to check if pending packets"),
                }
            }
        }
    }

    // In this case, the TxToken does all the work to transmit packets, hence the
    // simple function
    fn transmit(&'a mut self) -> Option<Self::TxToken> {
        let tx = TxToken {
            device: self,
            phantom: PhantomData,
        };

        Some(tx)
    }
}
```

With just this, we can write a program that sends/receives TCP packets. You can find an example in
`examples/bin/net`.

```rust
pub fn main() {
    // Assume we had setup declared all the other variables
    let enc28j60 = match enc28j60::Enc28j60::new(
        spi,
        ncs,
        enc28j60::Unconnected,
        reset,
        &mut delay,
        RX_BUF_SIZE,
        MAC_ADDR,
    ) {
        Ok(d) => d,
        Err(_) => panic!("Could not initialise driver"),
    };
    let device = Enc28j60::new(enc28j60);

    // Cache to store neighbors data
    let mut cache = [None; 4];
    let neighbor_cache = NeighborCache::new(&mut cache[..]);

    // Configure MAC address
    let ethernet_addr = EthernetAddress(MAC_ADDR);

    // Configure IP address and routes
    let ip_addr = IpAddress::v4(192, 168, 69, 1);
    let mut ip_addrs = [IpCidr::new(ip_addr, 24)];
    let default_v4_gw = Ipv4Address::new(192, 168, 69, 100);
    let mut routes_storage = [None; 1];
    let mut routes = Routes::new(&mut routes_storage[..]);
    routes.add_default_ipv4_route(default_v4_gw).unwrap();

    let mut storage = [SocketStorage::EMPTY; 2];
    // Initialise the interface
    let mut iface = InterfaceBuilder::new(device, &mut storage[..])
        .ip_addrs(&mut ip_addrs[..])
        .hardware_addr(ethernet_addr.into())
        .neighbor_cache(neighbor_cache)
        .finalize();

    // TCP socket
    let mut tx_buffer = [0; 2048];
    let mut rx_buffer = [0; 2048];
    let tcp_rx_buffer = TcpSocketBuffer::new(&mut rx_buffer[..]);
    let tcp_tx_buffer = TcpSocketBuffer::new(&mut tx_buffer[..]);
    let tcp_socket = TcpSocket::new(tcp_rx_buffer, tcp_tx_buffer);

    let tcp_handle = iface.add_socket(tcp_socket);
    let (socket, ctx) = iface.get_socket_and_context::<TcpSocket>(tcp_handle);
    socket
        .connect(ctx, (IpAddress::v4(192, 168, 69, 100), 7777), 49500)
        .unwrap();

    let mut tcp_active = false;
    loop {
        match iface.poll(Instant::now().into()) {
            Ok(_) => {}
            Err(e) => {
                defmt::debug!("poll error: {}", e);
            }
        }

        let socket = iface.get_socket::<TcpSocket>(tcp_handle);
        if socket.is_active() && !tcp_active {
            defmt::debug!("connected");
        } else if !socket.is_active() && tcp_active {
            panic!("disconnected");
        }
        tcp_active = socket.is_active();

        let msg = "hello";
        if socket.can_send() {
            socket.send_slice(msg.as_bytes()).unwrap();
            defmt::debug!("sent data")
        }
    }
}
```

Not too complicated - most of it is boilerplate. However, I ran into two issues running this successfully
(and they were entirely my fault). The first was that I misconfigured the MAC addresses. When you create
the ENC28J60 device, you pass in a MAC address as one of the arguments. Likewise, when configuring `smoltcp`,
you pass a MAC address into the interface definition via `hardware_addr`. When I first coded this, I
used two different MAC addresses. This was causing packets to get dropped.

The other problem was that I didn't allow traffic to that interface. My default iptables rules are
set to `DROP` (as they should be). So when the host was sending packets, nothing would be received
on the embedded system. Setting the iptables to accept all traffic to the interface solved the problem.

### Structuring the net stack

- Global stack so we have something that acts like a daemon
- Polling the interface for packets instead of interrupts
- Implementing the traits for TCP
