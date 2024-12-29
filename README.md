# Kiln
A modern build system for C/C++.

## Overview
If you're familiar with Rust's Cargo or Python's Poetry, you'll feel right at home with Kiln. This build system is designed to simplify and streamline the development process for C and C++ projects, making it accessible and efficient for both new and experienced developers.

### Why Choose Kiln?
- **Convenience**: Never write a makefile or link 20 files in a single compilation command again.
- **Security**: From better compiler flag defaults, to static analysis and valgrind integration, Kiln can help you write safer C/C++ code. 
- **Productivity**: Spend more time writing code and let Kiln take care of generating header files and linking libraries. 
- **Blazingly Fast Build Speeds**: Written in Rust, enough said :D
---

## Quick Start
### Creating a New Project
To create a new project directory and set up the build environment:
```bash
kiln new my_project
```
This will create a folder `my_project` with the following structure:
```
my_project/
├── src/
│   ├── main.c
│   └── utils.c
├── include/
│   └── utils.h
├── Kiln.toml
```

### Building Your Project
From the root of your project directory, run:
```bash
kiln build
```
Kiln will:
- Detect all source files in `src/`.
- Link necessary libraries.
- Output the compiled binary to the `build/` directory.

### Running Your Project
To compile and execute your project:
```bash
kiln run
```

### Valgrind Integration
Execute your project with valgrind and get info about memory corruption issues. 
```bash
kiln run --valgrind
```

### Generating Header Files
Automatically create/update your header files
```bash
kiln gen-headers
```

---

## Roadmap
- **Advanced Dependency Management**: Support for third-party library integration (e.g., Boost, OpenSSL).
- **Cross-Platform Builds**: Native support for Windows, macOS, and Linux.

---

## Contributing
We welcome contributions from the community! To contribute:
1. Fork the repository.
2. Create a new branch for your feature or bug fix.
3. Submit a pull request with a clear description of your changes.


## Support
- Submit an issue on GitHub: [GitHub Issues](https://github.com/akneni/kiln/issues)
