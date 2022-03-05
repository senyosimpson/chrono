@default:
  just --list

# Runs cargo clippy
check:
  cargo clippy --all-targets -- -A clippy::module_inception -A clippy::new_ret_no_self -A clippy::zero_ptr -A clippy::new_without_default

# Run cargo examples
example ex:
  cargo run --example {{ex}}

# List all examples
list-examples:
  #!/usr/bin/env python3

  import os
  import glob

  examples = glob.glob("examples/*.rs")
  for i, example in enumerate(examples, start=1):
    print(f"{i}: {os.path.basename(example)}")