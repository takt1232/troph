package com.sixdaggers.pocketfiles

import android.content.Context
import android.content.res.ColorStateList
import android.graphics.Color
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.graphics.drawable.RippleDrawable
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.widget.Button
import android.widget.LinearLayout
import android.widget.TextView
import java.util.Locale

object TrophTheme {
    const val BG = 0xFF0B0D10.toInt()
    const val SURFACE = 0xFF12151A.toInt()
    const val SURFACE_2 = 0xFF181C23.toInt()
    const val BORDER = 0xFF242A33.toInt()
    const val BORDER_STRONG = 0xFF333B47.toInt()
    const val TEXT = 0xFFE9EDF2.toInt()
    const val MUTED = 0xFF8B95A5.toInt()
    const val ACCENT = 0xFF4ADE80.toInt()
    const val ACCENT_STRONG = 0xFF22C55E.toInt()
    const val ON_ACCENT = 0xFF06120A.toInt()
    const val DANGER = 0xFFF87171.toInt()
    const val DANGER_BG = 0xFF221417.toInt()
    const val DANGER_BORDER = 0xFF4A2228.toInt()
}

fun Context.dp(value: Int): Int = (value * resources.displayMetrics.density).toInt()

fun Context.label(
    value: String,
    size: Float,
    color: Int = TrophTheme.TEXT,
    bold: Boolean = false,
    mono: Boolean = false,
): TextView = TextView(this).apply {
    text = value
    textSize = size
    setTextColor(color)
    if (bold) setTypeface(typeface, Typeface.BOLD)
    if (mono) typeface = Typeface.MONOSPACE
    setLineSpacing(0f, 1.25f)
}

fun Context.sectionTitle(value: String): TextView = label(value, 15f, TrophTheme.TEXT, bold = true)

fun Context.panel(): LinearLayout = LinearLayout(this).apply {
    orientation = LinearLayout.VERTICAL
    setPadding(dp(18), dp(18), dp(18), dp(18))
    background = GradientDrawable().apply {
        setColor(TrophTheme.SURFACE)
        cornerRadius = dp(16).toFloat()
        setStroke(dp(1), TrophTheme.BORDER)
    }
    layoutParams = LinearLayout.LayoutParams(
        ViewGroup.LayoutParams.MATCH_PARENT,
        ViewGroup.LayoutParams.WRAP_CONTENT,
    )
}

fun Context.rule(top: Int = 0, bottom: Int = 0): View = View(this).apply {
    setBackgroundColor(TrophTheme.BORDER)
    layoutParams = LinearLayout.LayoutParams(
        ViewGroup.LayoutParams.MATCH_PARENT,
        dp(1),
    ).apply {
        topMargin = dp(top)
        bottomMargin = dp(bottom)
    }
}

fun Context.dot(color: Int): GradientDrawable = GradientDrawable().apply {
    shape = GradientDrawable.OVAL
    setColor(color)
}

fun Context.rowParams(weight: Float = 1f): LinearLayout.LayoutParams =
    LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, weight).apply {
        marginEnd = dp(8)
    }

fun formatSize(bytes: Long): String {
    if (bytes <= 0) return "0 B"
    val units = arrayOf("B", "KB", "MB", "GB", "TB")
    var value = bytes.toDouble()
    var index = 0
    while (value >= 1024 && index < units.lastIndex) {
        value /= 1024
        index += 1
    }
    val rounded = if (value >= 100 || index == 0) {
        String.format(Locale.US, "%.0f", value)
    } else {
        String.format(Locale.US, "%.1f", value)
    }
    return "$rounded ${units[index]}"
}

fun Context.primaryButton(label: String, onClick: () -> Unit): Button =
    button(label, TrophTheme.ACCENT, TrophTheme.ON_ACCENT, null, onClick)

fun Context.secondaryButton(label: String, onClick: () -> Unit): Button =
    button(label, TrophTheme.SURFACE_2, TrophTheme.TEXT, TrophTheme.BORDER_STRONG, onClick)

fun Context.dangerButton(label: String, onClick: () -> Unit): Button =
    button(label, TrophTheme.DANGER_BG, TrophTheme.DANGER, TrophTheme.DANGER_BORDER, onClick)

fun Context.smallButton(label: String, onClick: () -> Unit): Button =
    button(label, TrophTheme.SURFACE_2, TrophTheme.TEXT, TrophTheme.BORDER_STRONG, onClick).apply {
        textSize = 13f
        minHeight = dp(38)
        minimumHeight = dp(38)
        minWidth = 0
        minimumWidth = 0
        setPadding(dp(12), 0, dp(12), 0)
        layoutParams = LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.WRAP_CONTENT,
            ViewGroup.LayoutParams.WRAP_CONTENT,
        )
    }

private fun Context.button(
    label: String,
    background: Int,
    foreground: Int,
    stroke: Int?,
    onClick: () -> Unit,
): Button = Button(this).apply {
    text = label
    textSize = 14f
    isAllCaps = false
    minHeight = dp(46)
    minimumHeight = dp(46)
    setTextColor(foreground)
    stateListAnimator = null
    backgroundTintList = null
    this.background = buttonBackground(background, stroke)
    setOnClickListener { onClick() }
    layoutParams = LinearLayout.LayoutParams(
        ViewGroup.LayoutParams.MATCH_PARENT,
        ViewGroup.LayoutParams.WRAP_CONTENT,
    )
}

private fun Context.buttonBackground(color: Int, stroke: Int?): RippleDrawable {
    val shape = GradientDrawable().apply {
        setColor(color)
        cornerRadius = dp(10).toFloat()
        if (stroke != null) setStroke(dp(1), stroke)
    }
    return RippleDrawable(ColorStateList.valueOf(0x33FFFFFF), shape, null)
}

class SettingRow(context: Context, title: String) : LinearLayout(context) {
    private val statusView = TextView(context).apply {
        textSize = 13f
        setTextColor(TrophTheme.MUTED)
    }
    private val actionView = context.smallButton("") {}.apply {
        visibility = GONE
    }

    init {
        orientation = HORIZONTAL
        gravity = Gravity.CENTER_VERTICAL
        setPadding(0, context.dp(10), 0, context.dp(10))
        addView(
            TextView(context).apply {
                text = title
                textSize = 14f
                setTextColor(TrophTheme.TEXT)
            },
            LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f),
        )
        addView(statusView, LinearLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT))
        addView(actionView, LinearLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT).apply {
            marginStart = context.dp(10)
        })
    }

    fun granted(message: String) {
        statusView.text = message
        statusView.visibility = VISIBLE
        actionView.visibility = GONE
    }

    fun action(label: String, onClick: () -> Unit) {
        actionView.text = label
        actionView.visibility = VISIBLE
        statusView.visibility = GONE
        actionView.setOnClickListener { onClick() }
    }
}
