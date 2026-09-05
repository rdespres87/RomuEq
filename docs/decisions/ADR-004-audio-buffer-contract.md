# ADR-004: Audio buffer contract

Status: Proposed

## MVP target

- f32;
- planar buffers;
- mono/stereo;
- variable block size;
- in-place processing where supported.

Invalid host buffer behavior must be defined before production RT code.
