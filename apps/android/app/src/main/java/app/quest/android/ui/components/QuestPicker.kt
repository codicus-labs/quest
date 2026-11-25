package app.quest.android.ui.components

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.input.TextFieldLineLimits
import androidx.compose.foundation.text.input.TextFieldState
import androidx.compose.foundation.text.input.rememberTextFieldState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
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
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import app.quest.android.R
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import uniffi.quest_mobile.QuestChip as QuestRef

/**
 * The quest [QuestChip] as the trigger of the quest picker: tap opens [QuestPickerSheet].
 * [quest] is the chosen one (null: "No quest"); [onPick] gets the new choice (null: [noneLabel]).
 */
@Composable
fun QuestPickerChip(
    quest: QuestRef?,
    options: List<QuestRef>,
    onPick: (QuestRef?) -> Unit,
    title: String,
    tag: String,
    modifier: Modifier = Modifier,
    noneLabel: String? = stringResource(R.string.no_quest),
) {
    var open by rememberSaveable { mutableStateOf(false) }
    Box(
        modifier
            .heightIn(min = QuestSpacing.touchTarget)
            .clickable(onClickLabel = title) { open = true }
            .semantics { contentDescription = "$title: ${quest?.name ?: noneLabel.orEmpty()}" }
            .testTag(tag),
        contentAlignment = Alignment.CenterStart,
    ) { QuestChip(quest) }
    if (open) {
        QuestPickerSheet(title, options, quest?.id, noneLabel, onPick = onPick, onDismiss = { open = false })
    }
}

/**
 * The quest picker as a bottom sheet (the desktop's quest combobox): a search field with the
 * keyboard up, then [options] in the core's order and [noneLabel] (if any) last, as on the
 * desktop. Picking closes it.
 */
@Composable
fun QuestPickerSheet(
    title: String,
    options: List<QuestRef>,
    selectedId: String?,
    noneLabel: String?,
    onPick: (QuestRef?) -> Unit,
    onDismiss: () -> Unit,
) {
    QuestBottomSheet(title, onDismiss) {
        QuestPickerContent(options, selectedId, noneLabel, rememberTextFieldState(), onPick = { quest ->
            onDismiss()
            onPick(quest)
        })
    }
}

/** The inside of [QuestPickerSheet] (also for screenshots: the modal sheet is a window of its own). */
@Composable
fun QuestPickerContent(
    options: List<QuestRef>,
    selectedId: String?,
    noneLabel: String?,
    query: TextFieldState,
    onPick: (QuestRef?) -> Unit,
    autoFocus: Boolean = true,
) {
    val focus = remember { FocusRequester() }
    if (autoFocus) LaunchedEffect(Unit) { focus.requestFocus() }
    val typed = query.text.toString()
    val matching = options.filter { matches(it.name, typed) }
    // Last, as on the desktop: the keyboard's Done takes the first quest that matches.
    val showNone = noneLabel != null && matches(noneLabel, typed)
    Column {
        OutlinedTextField(
            state = query,
            placeholder = { Text(stringResource(R.string.quest_picker_search)) },
            leadingIcon = { Icon(Icons.Filled.Search, contentDescription = null, tint = QuestTheme.colors.textMuted) },
            lineLimits = TextFieldLineLimits.SingleLine,
            // A fixed IME action: changing it while typing restarts the input and drops keys.
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
            // As Enter on the desktop: the first row that matches is taken.
            onKeyboardAction = {
                when {
                    typed.isBlank() -> Unit
                    matching.isNotEmpty() -> onPick(matching.first())
                    showNone -> onPick(null)
                }
            },
            modifier = Modifier.fillMaxWidth().focusRequester(focus).testTag("quest-picker-query"),
        )
        // Sized for the unfiltered list, so the sheet keeps its height while filtering.
        val rows = options.size + if (noneLabel != null) 1 else 0
        Box(Modifier.fillMaxWidth().padding(top = QuestSpacing.small).height(minOf(ListMaxHeight, QuestSpacing.touchTarget * rows))) {
            if (!showNone && matching.isEmpty()) {
                Text(
                    stringResource(R.string.quest_picker_empty),
                    style = QuestType.body,
                    color = QuestTheme.colors.textMuted,
                    modifier = Modifier.padding(vertical = QuestSpacing.medium).testTag("quest-picker-empty"),
                )
                return@Box
            }
            LazyColumn(Modifier.fillMaxSize().testTag("quest-picker-list")) {
                items(matching, key = { it.id }) { quest ->
                    PickerRow(quest.name, questColor(quest.colorHex), quest.id == selectedId, "quest-picker-option:${quest.name}") { onPick(quest) }
                }
                if (showNone) {
                    item(key = "none") {
                        PickerRow(noneLabel.orEmpty(), QuestTheme.colors.textFaint, selectedId == null, "quest-picker-none") { onPick(null) }
                    }
                }
            }
        }
    }
}

/** Like `quest_view::quest_picker::matches` (not called through UniFFI: Robolectric has no native lib). */
private fun matches(title: String, query: String) = title.contains(query.trim(), ignoreCase = true)

/** About seven rows; more scroll. */
private val ListMaxHeight = 360.dp

@Composable
private fun PickerRow(name: String, color: Color, selected: Boolean, tag: String, onClick: () -> Unit) {
    val colors = QuestTheme.colors
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = QuestSpacing.touchTarget)
            .selectable(selected = selected, role = Role.RadioButton, onClick = onClick)
            .padding(horizontal = QuestSpacing.small)
            .testTag(tag),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Dot(color, 8.dp)
        Text(
            name,
            style = QuestType.task,
            color = colors.text,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f).padding(start = QuestSpacing.medium),
        )
        if (selected) Icon(Icons.Filled.Check, contentDescription = null, tint = colors.accent, modifier = Modifier.size(20.dp))
    }
}

/** The filter icon (accent while filtering) that opens the quest picker; "All quests" clears the filter. */
@Composable
fun QuestFilterButton(filter: QuestRef?, options: List<QuestRef>, onFilter: (String?) -> Unit, tag: String) {
    var open by rememberSaveable { mutableStateOf(false) }
    val label = stringResource(R.string.plan_filter)
    val colors = QuestTheme.colors
    IconButton(onClick = { open = true }, modifier = Modifier.testTag(tag)) {
        Icon(QuestIcons.Filter, contentDescription = label, tint = if (filter != null) colors.accent else colors.textMuted)
    }
    if (open) {
        QuestPickerSheet(label, options, filter?.id, stringResource(R.string.plan_filter_all), onPick = { onFilter(it?.id) }, onDismiss = { open = false })
    }
}
