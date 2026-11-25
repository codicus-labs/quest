package app.quest.android.ui.task

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.isImeVisible
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.LinkAnnotation
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLinkStyles
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.withLink
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import app.quest.android.R
import app.quest.android.core.CoreHolder
import app.quest.android.core.Outcome
import app.quest.android.ui.components.CompleteToggle
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import uniffi.quest_mobile.MarkdownBlock
import uniffi.quest_mobile.MarkdownInline
import uniffi.quest_mobile.MarkdownInlineKind

internal fun CoreHolder.parseNote(source: String): List<MarkdownBlock> =
    (read { core, _ -> core.parseNote(source) } as? Outcome.Done)?.value.orEmpty()

internal fun CoreHolder.toggleNoteMarker(taskId: String, source: String, offset: ULong) =
    perform { core, now ->
        if (core.taskDetail(taskId, now).description == source) {
            val changed = core.toggleTaskMarker(source, offset)
            if (changed != source) core.setTaskDescription(taskId, changed)
        }
    }

class NoteActions(
    val edit: (String) -> Unit,
    val save: () -> Unit,
    val parse: (String) -> List<MarkdownBlock>,
    val toggleMarker: (String, ULong) -> Unit,
) {
    companion object {
        val None = NoteActions({}, {}, { emptyList() }, { _, _ -> })
    }
}

@Composable
internal fun TaskNote(taskId: String, source: String, actions: NoteActions, tag: String, modifier: Modifier = Modifier) {
    val colors = QuestTheme.colors
    val notes = stringResource(R.string.task_notes)
    var editing by rememberSaveable(taskId) { mutableStateOf(false) }
    if (!editing) {
        val blocks = remember(source) { if (source.isBlank()) emptyList() else actions.parse(source) }
        Column(
            modifier.fillMaxWidth().heightIn(min = QuestSpacing.touchTarget)
                .clickable(onClickLabel = stringResource(R.string.task_notes_edit)) { editing = true }
                .testTag(tag),
            verticalArrangement = Arrangement.spacedBy(QuestSpacing.small, Alignment.CenterVertically),
        ) {
            if (blocks.isEmpty()) Text(notes, style = QuestType.body, color = colors.textMuted)
            else MarkdownBlocks(blocks) { actions.toggleMarker(source, it) }
        }
        return
    }

    val requester = remember { FocusRequester() }
    val keyboard = LocalSoftwareKeyboardController.current
    var hadFocus by remember { mutableStateOf(false) }
    fun finish() {
        if (!editing) return
        editing = false
        actions.save()
        keyboard?.hide()
    }
    BackHandler(onBack = ::finish)
    val imeVisible = WindowInsets.isImeVisible
    var imeWasVisible by remember { mutableStateOf(false) }
    LaunchedEffect(imeVisible) {
        if (imeVisible) imeWasVisible = true
        else if (imeWasVisible) finish()
    }
    Column(modifier.fillMaxWidth()) {
        BasicTextField(
            source, actions.edit,
            textStyle = QuestType.body.copy(color = colors.text),
            cursorBrush = SolidColor(colors.accent),
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
            keyboardActions = KeyboardActions(onDone = { finish() }),
            decorationBox = { input ->
                Box(Modifier.fillMaxWidth().heightIn(min = QuestSpacing.touchTarget)) {
                    if (source.isEmpty()) Text(notes, style = QuestType.body, color = colors.textMuted)
                    input()
                }
            },
            modifier = Modifier.fillMaxWidth().testTag("$tag-source")
                .focusRequester(requester).onFocusChanged {
                    if (hadFocus && !it.isFocused) finish()
                    hadFocus = it.isFocused
                },
        )
        TextButton(onClick = ::finish, modifier = Modifier.align(Alignment.End)) {
            Text(stringResource(R.string.task_notes_done), color = colors.accent)
        }
    }
    LaunchedEffect(taskId) {
        requester.requestFocus()
        keyboard?.show()
    }
}

@Composable
private fun MarkdownBlocks(blocks: List<MarkdownBlock>, toggleMarker: (ULong) -> Unit) {
    val colors = QuestTheme.colors
    for (block in blocks) {
        when (block) {
            is MarkdownBlock.Heading -> MarkdownText(
                block.content,
                when (block.level.toInt()) {
                    1 -> QuestType.heading
                    2 -> QuestType.task
                    else -> QuestType.bodyMedium
                },
                Modifier.semantics { heading() },
            )
            is MarkdownBlock.Paragraph -> MarkdownText(block.content, QuestType.body)
            is MarkdownBlock.Code -> Box(
                Modifier.fillMaxWidth().background(colors.textMuted.copy(alpha = 0.08f))
                    .horizontalScroll(rememberScrollState()).padding(QuestSpacing.small),
            ) {
                Text(block.text, style = QuestType.body, color = colors.text, fontFamily = FontFamily.Monospace)
            }
            is MarkdownBlock.Quote -> Row {
                Box(Modifier.padding(end = QuestSpacing.small).border(1.dp, colors.borderStrong).padding(10.dp)) {
                    Column(verticalArrangement = Arrangement.spacedBy(QuestSpacing.small)) {
                        MarkdownBlocks(block.blocks, toggleMarker)
                    }
                }
            }
            is MarkdownBlock.ItemList -> Column {
                block.items.forEachIndexed { index, item ->
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        val marker = item.marker
                        if (marker != null) {
                            CompleteToggle(
                                marker.checked, { toggleMarker(marker.offset) }, Modifier.testTag("note-marker-${marker.offset}"),
                                uncheckedBorder = colors.textMuted,
                                square = true,
                            )
                        } else {
                            Text(
                                block.start?.let { "${it + index.toULong()}." } ?: "•",
                                style = QuestType.body, color = colors.textMuted,
                                modifier = Modifier.padding(horizontal = QuestSpacing.small),
                            )
                        }
                        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(QuestSpacing.small)) {
                            MarkdownBlocks(item.blocks, toggleMarker)
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun MarkdownText(content: List<MarkdownInline>, style: TextStyle, modifier: Modifier = Modifier) {
    val colors = QuestTheme.colors
    val text = remember(content, colors) {
        buildAnnotatedString {
            content.forEach { run ->
                withStyle(SpanStyle(
                    fontWeight = if (run.bold) FontWeight.Bold else null,
                    fontStyle = if (run.italic) FontStyle.Italic else null,
                    fontFamily = if (run.kind == MarkdownInlineKind.Code) FontFamily.Monospace else null,
                    background = if (run.kind == MarkdownInlineKind.Code) colors.textMuted.copy(alpha = 0.12f)
                    else Color.Unspecified,
                )) {
                    val kind = run.kind
                    if (kind is MarkdownInlineKind.Link) {
                        withLink(LinkAnnotation.Url(kind.url, TextLinkStyles(
                            style = SpanStyle(color = colors.accent, textDecoration = TextDecoration.Underline),
                        ))) { append(run.text) }
                    } else append(run.text)
                }
            }
        }
    }
    Text(text, style = style, color = colors.text, modifier = modifier)
}
