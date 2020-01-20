# core-os-riscv

An operating system in Rust.

This project is based on "The Adventures of OS: Making a RISC-V Operating System using Rust".
In files not modified by this project, original licenses are preserved.

## Build Instructions

First of all, install GNU RISC-V tools and QEMU. Python3 is also required to generate some files automatically.

```bash
brew tap riscv/riscv
brew install riscv-tools
brew test riscv-tools
brew install qemu
```

Don't forget to add riscv-tools to PATH.

Then, install Rust and related components.

```bash
cargo default nightly
cargo install cargo-xbuild cargo-binutils
rustup component add rust-src llvm-tools-preview rustfmt rls rust-analysis
rustup target add riscv64gc-unknown-none-elf
```

Finally you may build and run this project.

```bash
make qemu
```

If you want to use readelf tools, etc., you may install pwntools on macOS.

## Roadmap

The main goal of this project is to make an xv6-like operating system with the Rust programming language. And now it's in a very early stage. I'm still working on it.

- [x] Adapt code from http://osblog.stephenmarz.com/
- [x] UART drivers
- [x] Virtual Memory
- [x] Load ELF files from memory
- [x] Switch to User-mode
- [x] Process
- [ ] System call
- [ ] Scheduling
- [ ] Kernel Allocator
- [ ] Allocator and stdlib
- [ ] Real spinlock instead of nulllock
- [ ] Multi-core support
- [ ] Use Option instead of panic!
- [ ] Timer Interrupt and scheduling
- [ ] Persistence
- [ ] Eliminate use of unsafe
- [ ] Documentation
- [ ] High-level abstractions (driver, vm, etc.)
- [ ] Port to aarch64 and deploy on Raspi
- [ ] Rewrite code from other sources
- [ ] Security issues

## Reference

[1] https://github.com/rust-embedded/rust-raspi3-OS-tutorials

[2] https://github.com/bztsrc/raspi3-tutorial/

[3] https://os.phil-opp.com/

[4] http://osblog.stephenmarz.com/

[5] https://github.com/mit-pdos/xv6-riscv/

[6] https://pdos.csail.mit.edu/6.828/2012/labs

[7] https://gist.github.com/cb372/5f6bf16ca0682541260ae52fc11ea3bb

## Highlights of Rust-specific Implementations

* `swtch` takes one context, return another context, thus eliminating borrowing issues.
* `Process` takes full ownership of pagetable, context and trapframe.
