ENTRY(_start)

SECTIONS
{
  . = 0x400000;

  .text : {
    *(.text .text.*)
  }

  .rodata : {
    *(.rodata .rodata.*)
  }

  .data : {
    *(.data .data.*)
    *(.gcc_except_table)
  }

  .bss : {
    __bss_start = .;
    *(.bss .bss.*)
    *(COMMON)
    . = ALIGN(16);
    __bss_end = .;
  }

  /DISCARD/ : {
    *(.note .note.*)
    *(.eh_frame .eh_frame.*)
  }
}
