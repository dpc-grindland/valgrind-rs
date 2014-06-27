This project is a small collection of data structures for working with [Valgrind](http://valgrind.org) suppression files.

## Installation
This project uses the following software:

 *  [Rust](http://www.rust-lang.org)
 *  [Cargo](http://crates.io)

Valgrind itself is not technically a requirement, but it is recommended to install Valgrind anyway.

You will need the latest dev build of Rust. You can either download a nightly build of the Rust compiler and tools from http://www.rust-lang.org/install.html or clone the GitHub repository, [rust-lang/rust](https://github.com/rust-lang/rust), and [build from source](https://github.com/rust-lang/rust/#building-from-source). On Mac, it is recommended to use [Homebrew](http://brew.sh)'s `rust` formula:

<pre>
# first installation
brew install rust --HEAD

# update
brew reinstall rust --HEAD
</pre>

To install Cargo, you will need to build from source. See [Compiling cargo](https://github.com/rust-lang/cargo#compiling-cargo) for instructions. Homebrew users can automate the work of building from source using the `cargo` formula from the https://github.com/dtrebbien/homebrew-misc tap:

<pre>
# first installation
brew tap dtrebbien/misc
brew install cargo --HEAD

# update
brew reinstall cargo --HEAD
</pre>

With the dependencies installed, the `valgrind` crate is built by running:

<pre>
cargo build
</pre>

To generate the HTML documentation, run:

<pre>
rustdoc --output doc -w html src/valgrind.rs
</pre>

## License
The `valgrind` crate source code is licensed under the [GNU Lesser General Public License](http://www.gnu.org/licenses/lgpl.html), either version 3 of the LGPL, or (at your option) any later version.
