package rs.compose.android

import android.graphics.Rect
import android.view.View
import android.view.ViewStructure
import android.view.autofill.AutofillManager
import android.view.autofill.AutofillValue
import java.nio.ByteBuffer
import java.nio.ByteOrder

/**
 * The platform's autofill for a [ComposeRsView] (D1687), as Compose's `AndroidAutofillManager`'s
 * platform side: Rust's tree decides what to tell (`autofill.rs` in compose-ui) and this tells
 * Android's [AutofillManager]; the virtual structure Rust writes (`autofill.rs` in
 * compose-android) is written into the framework's [ViewStructure]; the service's values go back
 * to Rust.
 */
internal class ComposeRsAutofill(private val view: View) {
    private val manager: AutofillManager? by lazy {
        view.context.getSystemService(AutofillManager::class.java)
    }

    /** Rust's call ([ComposeRsView.autofillCall]): one of `autofill::kind`'s. */
    fun call(kind: Int, id: Int, bounds: Rect, valueType: Int, text: String?, number: Long) {
        val manager = manager ?: return
        when (kind) {
            VIEW_ENTERED -> manager.notifyViewEntered(view, id, bounds)
            VIEW_EXITED -> manager.notifyViewExited(view, id)
            VALUE_CHANGED -> value(valueType, text, number)?.let { manager.notifyValueChanged(view, id, it) }
            VISIBILITY_CHANGED -> manager.notifyViewVisibilityChanged(view, id, number != 0L)
            COMMIT -> manager.commit()
            CANCEL -> manager.cancel()
            REQUEST -> manager.requestAutofill(view, id, bounds)
        }
    }

    /**
     * Writes Rust's structure into [root], as Compose's `populateViewStructure`: the root's
     * fields, then each child added under its parent.
     */
    fun populate(root: ViewStructure, bytes: ByteBuffer) {
        bytes.order(ByteOrder.LITTLE_ENDIAN)
        val rootId = view.autofillId ?: return
        node(root, bytes, rootId)
    }

    private fun node(structure: ViewStructure, bytes: ByteBuffer, rootId: android.view.autofill.AutofillId) {
        val id = bytes.getInt()
        val flags = bytes.getInt()
        val autofillType = bytes.getInt()
        val inputType = bytes.getInt()
        val maxTextLength = bytes.getInt()
        val visibility = bytes.getInt()
        structure.setAutofillId(rootId, id)
        structure.setId(id, view.context.packageName, null, null)
        if (autofillType != View.AUTOFILL_TYPE_NONE) structure.setAutofillType(autofillType)
        if (flags and HAS_BOUNDS != 0) {
            val left = bytes.getInt()
            val top = bytes.getInt()
            val right = bytes.getInt()
            val bottom = bytes.getInt()
            structure.setDimens(left, top, 0, 0, right - left, bottom - top)
        }
        if (flags and HAS_VALUE != 0) {
            val type = bytes.getInt()
            val text = bytes.readString()
            val number = bytes.getLong()
            value(type, text, number)?.let { structure.setAutofillValue(it) }
        }
        val hints = Array(bytes.getInt()) { bytes.readString() }
        if (hints.isNotEmpty()) structure.setAutofillHints(hints)
        if (flags and HAS_CLASS_NAME != 0) structure.setClassName(bytes.readString())
        if (flags and HAS_DESCRIPTION != 0) structure.setContentDescription(bytes.readString())
        val text = bytes.readString()
        if (text.isNotEmpty()) structure.setText(text)
        if (flags and CHECKABLE != 0) structure.setCheckable(true)
        if (flags and CHECKED != 0) structure.setChecked(true)
        if (flags and FOCUSABLE != 0) structure.setFocusable(true)
        if (flags and FOCUSED != 0) structure.setFocused(true)
        if (flags and CLICKABLE != 0) structure.setClickable(true)
        if (flags and LONG_CLICKABLE != 0) structure.setLongClickable(true)
        if (flags and SELECTED != 0) structure.setSelected(true)
        if (flags and ENABLED == 0) structure.setEnabled(false)
        structure.setDataIsSensitive(flags and SENSITIVE != 0)
        if (inputType != 0) structure.setInputType(inputType)
        if (maxTextLength >= 0) structure.setMaxTextLength(maxTextLength)
        structure.setVisibility(visibility)
        val children = bytes.getInt()
        if (children == 0) return
        val first = structure.addChildCount(children)
        for (index in 0 until children) node(structure.newChild(first + index), bytes, rootId)
    }

    private fun ByteBuffer.readString(): String {
        val bytes = ByteArray(getInt())
        get(bytes)
        return String(bytes, Charsets.UTF_8)
    }

    companion object {
        // `autofill::kind`'s.
        const val VIEW_ENTERED = 0
        const val VIEW_EXITED = 1
        const val VALUE_CHANGED = 2
        const val VISIBILITY_CHANGED = 3
        const val COMMIT = 4
        const val CANCEL = 5
        const val REQUEST = 6

        // `autofill::flag`'s.
        const val CHECKABLE = 1
        const val FOCUSABLE = 1 shl 1
        const val SENSITIVE = 1 shl 2
        const val ENABLED = 1 shl 3
        const val CHECKED = 1 shl 4
        const val CLICKABLE = 1 shl 5
        const val FOCUSED = 1 shl 6
        const val LONG_CLICKABLE = 1 shl 7
        const val SELECTED = 1 shl 8
        const val HAS_BOUNDS = 1 shl 9
        const val HAS_VALUE = 1 shl 10
        const val HAS_CLASS_NAME = 1 shl 11
        const val HAS_DESCRIPTION = 1 shl 12

        /** A value as it crosses (`autofill::value_parts`), Android's. */
        fun value(type: Int, text: String?, number: Long): AutofillValue? =
            when (type) {
                View.AUTOFILL_TYPE_TEXT -> AutofillValue.forText(text ?: "")
                View.AUTOFILL_TYPE_TOGGLE -> AutofillValue.forToggle(number != 0L)
                View.AUTOFILL_TYPE_LIST -> AutofillValue.forList(number.toInt())
                View.AUTOFILL_TYPE_DATE -> AutofillValue.forDate(number)
                else -> null
            }

        /** Android's value as it crosses to Rust: its type, text and number. */
        fun parts(value: AutofillValue): Triple<Int, String?, Long> =
            when {
                value.isText -> Triple(View.AUTOFILL_TYPE_TEXT, value.textValue.toString(), 0L)
                value.isToggle -> Triple(View.AUTOFILL_TYPE_TOGGLE, null, if (value.toggleValue) 1L else 0L)
                value.isList -> Triple(View.AUTOFILL_TYPE_LIST, null, value.listValue.toLong())
                value.isDate -> Triple(View.AUTOFILL_TYPE_DATE, null, value.dateValue)
                else -> Triple(View.AUTOFILL_TYPE_NONE, null, 0L)
            }
    }
}
