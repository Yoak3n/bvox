package dev.yoa.bvox.lyrics

/**
 * One rendered lyric step.
 *
 * [primary] is the line itself; [translation] is a second paragraph of a
 * bilingual subtitle (or null); [next] is the following non-empty line, used
 * as the second row when the subtitle has no translation.
 */
internal data class LyricUiLine(
    val primary: String,
    val translation: String?,
    val next: String?,
)
