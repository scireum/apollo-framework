# Apollo Framework

The Apollo framework is essentailly a runtime and a toolkit for building stable and robust server software.

 **Apollo** was extracted out of [Jupiter](https://github.com/scireum/jupiter) to make its
core parts usable by other libraries or applications.

Apollo provides a small **dependency injection helper** called **Platform** along with some
tooling to setup logging, format messages and to react on signals. This is commonly required
to properly run inside a Docker container.

Most notably a **Config** and a **Server** component is provided, both fully **aync** as they
built up on [Tokio](https://tokio.rs).

The **Config** monitors a local config file and broadcasts a change event throughout the whole
platform once a new configuration is available.

The **Server** opens a port and spins of a loop which accepts and handles connections. The
protocol implementation to be applied to each connection can be passed in using a closure
or function reference.

Note that the server also monitors the config and will re-allocate a new port if the underlying
config changes. This permits for zero downtime migrations during releases.

## Links
* Documentation: [docs.rs](https://docs.rs/apollo-framework)
* Repository: [GitHub](https://github.com/scireum/apollo-framework)
* Crate: [crates.io](https://crates.io/crates/apollo-framework)

## Contributions

Contributions as issues or pull requests are always welcome. Please [sign-off](http://developercertificate.org)
all your commits by adding a line like "Signed-off-by: Name <email>" at the end of each commit, indicating that
you wrote the code and have the right to pass it on as an open source.

## License

The **Apollo framework** is licensed under the MIT License:

> Permission is hereby granted, free of charge, to any person obtaining a copy
> of this software and associated documentation files (the "Software"), to deal
> in the Software without restriction, including without limitation the rights
> to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
> copies of the Software, and to permit persons to whom the Software is
> furnished to do so, subject to the following conditions:
>
> The above copyright notice and this permission notice shall be included in
> all copies or substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
> IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
> FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
> AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
> LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
> OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
> THE SOFTWARE.
