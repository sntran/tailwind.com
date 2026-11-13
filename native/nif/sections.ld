/* For the native NIF objects (scripts/nif.sh, with scripts/rust_link.sh):
 * one output section for each kind, in place of one section for each
 * function and each constant (about 36,000). The object is then 14 MB,
 * not 22 MB: each section has a header and a name. The loader of beam.com
 * keeps the sections that the code uses, but LTO already removed the
 * code that the crate does not use. And no section has the size 0: the
 * loader of beam.com does not load such a section, and a static of Rust
 * with a type of size 0 has one. */
SECTIONS
{
  .text : { *(.text .text.*) }
  .rodata : { *(.rodata .rodata.*) }
  .data.rel.ro : { *(.data.rel.ro .data.rel.ro.*) }
  .data : { *(.data .data.*) }
  .bss : { *(.bss .bss.*) }
  .tdata : { *(.tdata .tdata.*) }
  .tbss : { *(.tbss .tbss.*) }
  .gcc_except_table : { *(.gcc_except_table .gcc_except_table.*) }
}
