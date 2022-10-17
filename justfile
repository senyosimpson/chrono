@default:
  just --list

# Runs cargo clippy
check:
  cargo clippy --all-targets -- -A clippy::module_inception -A clippy::new_ret_no_self -A clippy::zero_ptr -A clippy::new_without_default

# Setup network interface
setup-interface interface:
  sudo iptables -A INPUT -i {{interface}} -j ACCEPT
  sudo iptables -A OUTPUT -o {{interface}} -j ACCEPT
  
  sudo ip addr add 192.168.69.100/24 dev {{interface}}
  

# Socat listener
socat-listen:
  socat TCP-LISTEN:7777 STDOUT

# Run cargo examples
example ex:
  DEFMT_LOG=debug cargo run --example {{ex}}

# Print output of Rust macro
expand-macro ex:
  cargo expand --example {{ex}}

# List all examples
list-examples:
  #!/usr/bin/env python3

  import os
  import glob

  examples = glob.glob("examples/*.rs")
  for i, example in enumerate(examples, start=1):
    print(f"{i}: {os.path.basename(example)}")
