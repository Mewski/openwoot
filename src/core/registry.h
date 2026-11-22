/*
 * core: registration lists. A record is defined next to the code it names, with a
 * macro that places it in a named list; the linker collects each list into one array.
 * There is no central file that names the contributors and no code that runs to
 * register.
 *
 * A list is declared once, in the header of the module that owns it:
 *     REGISTRY_DECLARE(profile_sections, profile_section_t);
 * a contributor adds an entry in any of its .c files through a macro of the owner
 * built on REGISTRY_ENTRY, and the owner walks the list with REGISTRY_FOREACH, or with
 * REGISTRY_FOREACH_ORDERED for a list whose entry type has a uint16_t field `order`.
 *
 * Layout: the entries of list L lie in the sections .registry.L.1, between the empty
 * markers in .registry.L.0 and .registry.L.2. The line KEEP(*(SORT(.registry.*))) in
 * every family's sections.ld sorts them by section name, so each list is contiguous
 * and lies between its two markers. A new list needs no change of a linker script.
 *
 * Depends on: the compiler (GCC section attributes) and that line of sections.ld.
 */
#ifndef CORE_REGISTRY_H_
#define CORE_REGISTRY_H_

#include <stddef.h>
#include <stdint.h>

/* Declares list `list` with entries of type `type`: its begin and end markers. */
#define REGISTRY_DECLARE(list, type)                                                         \
    static const type registry_##list##_begin[0]                                              \
        __attribute__((used, section(".registry." #list ".0")));                              \
    static const type registry_##list##_end[0]                                                \
        __attribute__((used, section(".registry." #list ".2")))

/* Starts the definition of entry `id` of list `list`; the initializer follows. */
#define REGISTRY_ENTRY(list, type, id) \
    static const type registry_##list##_##id __attribute__((used, section(".registry." #list ".1")))

/* Walks list `list` in link order; `entry` is a `const type *`. */
#define REGISTRY_FOREACH(list, type, entry)                                                   \
    for (const type *entry = registry_hide_origin(registry_##list##_begin),                   \
                    *const entry##_end = registry_hide_origin(registry_##list##_end);          \
         entry < entry##_end; entry++)

/*
 * Walks list `list` in ascending `order`; `entry` is a `const type *`. Of several
 * entries with the same order number only the first in link order is taken.
 */
#define REGISTRY_FOREACH_ORDERED(list, type, entry)                                           \
    for (const type *entry = registry_next_ordered(registry_hide_origin(registry_##list##_begin), \
                                                   registry_hide_origin(registry_##list##_end),   \
                                                   sizeof(type), offsetof(type, order), NULL);    \
         entry != NULL;                                                                           \
         entry = registry_next_ordered(registry_hide_origin(registry_##list##_begin),             \
                                       registry_hide_origin(registry_##list##_end),               \
                                       sizeof(type), offsetof(type, order), entry))

/*
 * Returns the address of a marker without telling the compiler which object it
 * belongs to: the entries lie outside the (empty) marker array, so the walk must not
 * be derived from its bounds.
 */
static inline const void *registry_hide_origin(const void *address)
{
    __asm__("" : "+r"(address));
    return address;
}

/*
 * Returns the first entry in [begin, end) whose order number is the smallest one
 * above that of `previous` (any order number if `previous` is NULL), or NULL.
 */
static inline const void *registry_next_ordered(const void *begin, const void *end, size_t size,
                                                size_t order_offset, const void *previous)
{
    const uint8_t *next = NULL;
    uint16_t next_order = 0u;

    for (const uint8_t *entry = begin; entry < (const uint8_t *)end; entry += size) {
        uint16_t order = *(const uint16_t *)(const void *)(entry + order_offset);

        if (previous != NULL && order <= *(const uint16_t *)(const void *)((const uint8_t *)previous + order_offset)) {
            continue;
        }
        if (next == NULL || order < next_order) {
            next = entry;
            next_order = order;
        }
    }
    return next;
}

#endif /* CORE_REGISTRY_H_ */
