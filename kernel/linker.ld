ENTRY(_start)

/* Явно указываем сегменты, чтобы Линковщик не путался с правами */
PHDRS
{
    text    PT_LOAD    FLAGS(5); /* 1(Execute) + 4(Read) = 5 */
    rodata  PT_LOAD    FLAGS(4); /* 4(Read)              = 4 */
    data    PT_LOAD    FLAGS(6); /* 2(Write)   + 4(Read) = 6 */
}

SECTIONS
{
    . = 0xffffffff80200000;

    /* 1. КОД (RX) */
    .text : {
        *(.text .text.*)
    } :text

    /* !!! КРИТИЧЕСКИ ВАЖНО !!! */
    /* Переходим на новую страницу, чтобы сменить права доступа */
    . = ALIGN(0x1000);

    /* 2. КОНСТАНТЫ И ЗАПРОСЫ (R) */
    .rodata : {
        *(.rodata .rodata.*)
    } :rodata

    /* Запросы тоже Read-Only, кладем их сюда */
    .requests : {
        KEEP(*(.requests))
    } :rodata

    /* !!! КРИТИЧЕСКИ ВАЖНО !!! */
    /* Снова новая страница для записываемых данных */
    . = ALIGN(0x1000);

    /* 3. ДАННЫЕ (RW) */
    .data : {
        *(.data .data.*)
    } :data

    .bss : {
        *(.bss .bss.*)
        *(COMMON)
    } :data

    /DISCARD/ : {
        *(.eh_frame)
        *(.note .note.*)
    }
}