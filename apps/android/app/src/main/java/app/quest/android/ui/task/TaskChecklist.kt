package app.quest.android.ui.task

import android.view.inputmethod.InputConnectionWrapper
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.gestures.detectDragGesturesAfterLongPress
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.layout.positionInParent
import androidx.compose.ui.platform.InterceptPlatformTextInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.PlatformTextInputInterceptor
import androidx.compose.ui.platform.PlatformTextInputMethodRequest
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import app.quest.android.R
import app.quest.android.ui.components.CompleteToggle
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import kotlin.math.roundToInt
import uniffi.quest_mobile.ChecklistRow

class ChecklistActions(
    val toggle: (String, Boolean) -> Unit = { _, _ -> },
    val add: (String) -> String? = { null },
    val insertAfter: (String) -> String? = { null },
    val rename: (String, String) -> Unit = { _, _ -> },
    val delete: (String) -> Unit = {},
    val move: (String, String?) -> Unit = { _, _ -> },
)

private data class ChecklistDrag(val item: ChecklistRow, val height: Float, val grabY: Float, val top: Float, val index: Int)
private val RowShape = RoundedCornerShape(10.dp)

@Composable
internal fun TaskChecklist(
    taskId: String,
    items: List<ChecklistRow>,
    actions: ChecklistActions,
    editable: Boolean,
    modifier: Modifier = Modifier,
) {
    val colors = QuestTheme.colors
    val density = LocalDensity.current
    val haptics = LocalHapticFeedback.current
    val bounds = remember(taskId) { mutableMapOf<String, Rect>() }
    val currentItems by rememberUpdatedState(items)
    val currentActions by rememberUpdatedState(actions)
    var drag by remember(taskId) { mutableStateOf<ChecklistDrag?>(null) }
    var focusItem by remember(taskId) { mutableStateOf<String?>(null) }
    val gestures = if (!editable) Modifier else Modifier.pointerInput(taskId, haptics) {
        detectDragGesturesAfterLongPress(
            onDragStart = { position ->
                val index = currentItems.indexOfFirst { bounds[it.id]?.contains(position) == true }
                currentItems.getOrNull(index)?.let { item ->
                    val row = bounds.getValue(item.id)
                    drag = ChecklistDrag(item, row.height, position.y - row.top, row.top, index)
                    haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                }
            },
            onDrag = { change, amount ->
                drag?.let { lifted ->
                    change.consume()
                    val target = currentItems.filterNot { it.id == lifted.item.id }.indexOfFirst {
                        val row = bounds[it.id] ?: return@indexOfFirst false
                        change.position.y < row.center.y
                    }
                    drag = lifted.copy(
                        top = lifted.top + amount.y,
                        index = if (target < 0) currentItems.lastIndex.coerceAtLeast(0) else target,
                    )
                }
            },
            onDragCancel = { drag = null },
            onDragEnd = {
                drag?.let { lifted ->
                    val original = currentItems.indexOfFirst { it.id == lifted.item.id }
                    if (original >= 0 && original != lifted.index) {
                        val before = currentItems.filterNot { it.id == lifted.item.id }.getOrNull(lifted.index)
                        currentActions.move(lifted.item.id, before?.id)
                    }
                }
                drag = null
            },
        )
    }
    val lifted = drag?.takeIf { drag -> items.any { it.id == drag.item.id } }
    val ordered = if (lifted == null) items else items.filterNot { it.id == lifted.item.id }.toMutableList().apply {
        add(lifted.index.coerceIn(0, size), lifted.item)
    }
    Box(modifier.fillMaxWidth()) {
        Column(Modifier.fillMaxWidth().then(gestures)) {
            ordered.forEach { item ->
                key(item.id) {
                    if (lifted?.item?.id == item.id) {
                        Box(Modifier.fillMaxWidth().height(with(density) { lifted.height.toDp() })
                            .background(colors.textMuted.copy(alpha = 0.08f), RowShape)
                            .border(1.dp, colors.borderStrong, RowShape).testTag("checklist-drop-gap"))
                    } else {
                        ChecklistLine(
                            item, actions, editable, focusItem == item.id,
                            onFocused = { focusItem = null },
                            onInsert = { actions.insertAfter(item.id)?.let { focusItem = it } },
                            onDelete = {
                                focusItem = items.takeWhile { it.id != item.id }.lastOrNull()?.id
                                    ?: items.getOrNull(1)?.id
                                actions.delete(item.id)
                            },
                            modifier = Modifier.onGloballyPositioned {
                                val origin = it.positionInParent()
                                bounds[item.id] = Rect(origin.x, origin.y, origin.x + it.size.width, origin.y + it.size.height)
                            },
                        )
                    }
                }
            }
            if (editable) AddChecklistLine(actions.add)
        }
        lifted?.let {
            ChecklistLine(it.item, ChecklistActions(), editable = false, requestFocus = false,
                onFocused = {}, onInsert = {}, onDelete = {},
                modifier = Modifier.offset { IntOffset(0, it.top.roundToInt()) }
                    .shadow(8.dp, RowShape).background(colors.card, RowShape).clearAndSetSemantics {},
            )
        }
    }
}

@Composable
private fun ChecklistLine(
    item: ChecklistRow,
    actions: ChecklistActions,
    editable: Boolean,
    requestFocus: Boolean,
    onFocused: () -> Unit,
    onInsert: () -> Unit,
    onDelete: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val colors = QuestTheme.colors
    val style = QuestType.body.copy(
        color = if (item.done) colors.textMuted else colors.text,
        textDecoration = if (item.done) TextDecoration.LineThrough else null,
    )
    Row(modifier.fillMaxWidth().heightIn(min = QuestSpacing.touchTarget), verticalAlignment = Alignment.CenterVertically) {
        CompleteToggle(
            item.done, { actions.toggle(item.id, !item.done) }, Modifier.testTag("checklist-toggle-${item.id}"),
            uncheckedBorder = colors.textMuted,
            square = true,
            label = item.text.ifBlank { stringResource(R.string.task_checklist_item) },
        )
        if (!editable) {
            Text(item.text, style = style, modifier = Modifier.weight(1f))
            return@Row
        }
        var draft by remember { mutableStateOf<String?>(null) }
        val value = draft ?: item.text
        val requester = remember { FocusRequester() }
        val label = stringResource(R.string.task_checklist_item)
        fun save() {
            draft?.let { actions.rename(item.id, it) }
            draft = null
        }
        fun insert() {
            save()
            onInsert()
        }
        fun removeIfEmpty(): Boolean {
            if ((draft ?: item.text).isNotEmpty()) return false
            draft = null
            onDelete()
            return true
        }
        EmptyBackspace(::removeIfEmpty) {
            BasicTextField(
                value, { draft = it },
                textStyle = style,
                singleLine = true,
                cursorBrush = SolidColor(colors.accent),
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Next),
                keyboardActions = KeyboardActions(onNext = { insert() }),
                decorationBox = { input ->
                    Box(Modifier.heightIn(min = QuestSpacing.touchTarget), contentAlignment = Alignment.CenterStart) { input() }
                },
                modifier = Modifier.weight(1f).focusRequester(requester)
                    .onFocusChanged { if (!it.isFocused) save() }
                    .onPreviewKeyEvent {
                        if (it.type != KeyEventType.KeyDown) return@onPreviewKeyEvent false
                        when {
                            it.key == Key.Enter || it.key == Key.NumPadEnter -> { insert(); true }
                            it.key == Key.Backspace -> removeIfEmpty()
                            else -> false
                        }
                    }
                    .semantics { contentDescription = label }
                    .testTag("checklist-text-${item.id}"),
            )
        }
        val commit by rememberUpdatedState(::save)
        DisposableEffect(item.id) { onDispose { commit() } }
        LaunchedEffect(requestFocus) {
            if (requestFocus) {
                requester.requestFocus()
                onFocused()
            }
        }
    }
}

@Composable
private fun EmptyBackspace(removeIfEmpty: () -> Boolean, content: @Composable () -> Unit) {
    val remove by rememberUpdatedState(removeIfEmpty)
    val interceptor = remember {
        PlatformTextInputInterceptor { request, session ->
            session.startInputMethod(PlatformTextInputMethodRequest { info ->
                object : InputConnectionWrapper(request.createInputConnection(info), false) {
                    override fun deleteSurroundingText(beforeLength: Int, afterLength: Int): Boolean =
                        (beforeLength > 0 && remove()) || super.deleteSurroundingText(beforeLength, afterLength)

                    override fun deleteSurroundingTextInCodePoints(beforeLength: Int, afterLength: Int): Boolean =
                        (beforeLength > 0 && remove()) || super.deleteSurroundingTextInCodePoints(beforeLength, afterLength)
                }
            })
        }
    }
    InterceptPlatformTextInput(interceptor, content)
}

@Composable
private fun AddChecklistLine(add: (String) -> String?) {
    val colors = QuestTheme.colors
    val label = stringResource(R.string.task_checklist_add)
    var text by remember { mutableStateOf("") }
    fun submit() {
        if (text.isNotBlank() && add(text) != null) text = ""
    }
    BasicTextField(
        text, { text = it }, singleLine = true,
        textStyle = QuestType.body.copy(color = colors.text),
        cursorBrush = SolidColor(colors.accent),
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
        keyboardActions = KeyboardActions(onDone = { submit() }),
        modifier = Modifier.fillMaxWidth().onPreviewKeyEvent {
            if (it.type == KeyEventType.KeyDown && (it.key == Key.Enter || it.key == Key.NumPadEnter)) {
                submit()
                true
            } else false
        }.semantics { contentDescription = label }.testTag("checklist-add"),
        decorationBox = { input ->
            Row(Modifier.heightIn(min = QuestSpacing.touchTarget), verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.padding(horizontal = 16.dp), contentAlignment = Alignment.Center) {
                    Text("+", style = QuestType.heading, color = colors.textMuted)
                }
                Box(Modifier.weight(1f)) {
                    if (text.isEmpty()) Text(label, style = QuestType.body, color = colors.textMuted)
                    input()
                }
            }
        },
    )
}
