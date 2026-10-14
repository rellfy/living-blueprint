# Rules

- Before writing any code, read CONTRIBUTING.md.
- Do not modify README.md (or these rules!).

# Architecture Overview

The program is a native GUI built on winit and wgpu, with vello for vector
rendering and parley for text layout.

The codebase is structured around a few architecture primitives:

- State
- Resource
- System

State refers to the global program state which owns all the things, i.e.
it owns all the resources and system states.
Resources are stateful objects that only have one instance for the lifetime of
the program.
Systems are functions that operate on the state.

Systems can have their own state. A system's state is technically a resource,
but it is located in the system module rather than in the resources module
to make things more isolated and organised.
A resource in the resources module is, therefore, used by multiple systems,
otherwise it would be part of a system's state.

