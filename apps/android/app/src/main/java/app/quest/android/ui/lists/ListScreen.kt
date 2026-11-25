package app.quest.android.ui.lists

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.wrapContentWidth
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import app.quest.android.R
import app.quest.android.ui.components.HoursRing
import app.quest.android.ui.components.QuestIcons
import app.quest.android.ui.components.UndoSnackbar
import app.quest.android.ui.components.questColor
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import uniffi.quest_mobile.ListKind
import uniffi.quest_mobile.ListSection
import uniffi.quest_mobile.TaskList

/** The widest a list grows (tablets); centred beyond. */
val ListMaxWidth = 720.dp

class ListActions(
    val rows: RowActions,
    val editDraft: (String) -> Unit,
    val add: () -> Unit,
    val undo: () -> Unit,
    val dismissToast: () -> Unit,
    val quick: QuickAddActions = QuickAddActions(),
)

/**
 * One list: [top] (Today's segment, a quest's way back), the header, quick add (not on the Logbook),
 * the open rows, then "n done". The core's undo toast floats at the bottom.
 */
@Composable
fun ListScreen(
    state: ListUiState,
    actions: ListActions,
    modifier: Modifier = Modifier,
    top: @Composable () -> Unit = {},
    headerActions: @Composable () -> Unit = {},
) {
    when (state) {
        ListUiState.Loading -> Box(modifier.fillMaxSize(), contentAlignment = Alignment.Center) { CircularProgressIndicator() }
        ListUiState.Gone -> Note(stringResource(R.string.list_gone), modifier)
        is ListUiState.Failed -> Note(state.message, modifier)
        is ListUiState.Ready -> Box(modifier.fillMaxSize()) {
            ListContent(state, actions, top, headerActions)
            state.list.toast?.let { toast ->
                UndoSnackbar(
                    toast.text,
                    onUndo = actions.undo,
                    actionLabel = toast.actionLabel,
                    onDismiss = actions.dismissToast,
                    modifier = Modifier.align(Alignment.BottomCenter).widthIn(max = 560.dp),
                )
            }
        }
    }
}

@Composable
private fun ListContent(state: ListUiState.Ready, actions: ListActions, top: @Composable () -> Unit, headerActions: @Composable () -> Unit) {
    val list = state.list
    LazyColumn(
        Modifier.fillMaxSize().wrapContentWidth(Alignment.CenterHorizontally).widthIn(max = ListMaxWidth).testTag("list-${list.kind.tag}"),
        contentPadding = PaddingValues(start = QuestSpacing.medium, end = QuestSpacing.medium, bottom = 88.dp),
    ) {
        item(key = "top") { Box(Modifier.padding(horizontal = QuestSpacing.small)) { top() } }
        item(key = "header") { ListHeader(list, headerActions) }
        if (list.kind != ListKind.Logbook) {
            item(key = "add") { ListQuickAdd(state.draft, state.quick, actions) }
        }
        if (list.empty) item(key = "empty") { EmptyList(list.kind) }
        list.sections.forEachIndexed { index, section -> section(index, section, actions.rows) }
    }
}

private fun androidx.compose.foundation.lazy.LazyListScope.section(index: Int, section: ListSection, actions: RowActions) {
    section.label?.let { label ->
        item(key = "label-$index") {
            Text(
                label,
                style = QuestType.caps,
                color = QuestTheme.colors.textMuted,
                modifier = Modifier.padding(start = QuestSpacing.small, top = QuestSpacing.section, bottom = QuestSpacing.tight).semantics { heading() },
            )
        }
    }
    items(section.rows, key = { "$index-${it.id}" }) { row -> TaskRow(row, actions) }
}

/** Short and stable, for test tags: today, inbox, logbook, quest. */
val ListKind.tag: String
    get() = when (this) {
        ListKind.Today -> "today"
        ListKind.Inbox -> "inbox"
        ListKind.Logbook -> "logbook"
        is ListKind.Quest -> "quest"
    }

@Composable
private fun ListHeader(list: TaskList, actions: @Composable () -> Unit) {
    val colors = QuestTheme.colors
    Row(
        Modifier.fillMaxWidth().padding(start = QuestSpacing.small, top = QuestSpacing.small, bottom = QuestSpacing.medium),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(QuestSpacing.medium),
    ) {
        val quest = list.quest
        when {
            quest != null -> HoursRing(list.ring, questColor(quest.colorHex), size = 28.dp, stroke = 4.dp)
            else -> kindIcon(list.kind)?.let { (icon, tint) -> Icon(icon, contentDescription = null, tint = tint, modifier = Modifier.size(28.dp)) }
        }
        Column(Modifier.weight(1f)) {
            Text(list.title, style = QuestType.heading, color = colors.text, maxLines = 2, overflow = TextOverflow.Ellipsis, modifier = Modifier.semantics { heading() })
            if (list.subtitle.isNotEmpty()) Text(list.subtitle, style = QuestType.small, color = colors.textMuted)
        }
        actions()
    }
}

@Composable
private fun kindIcon(kind: ListKind): Pair<ImageVector, Color>? {
    val colors = QuestTheme.colors
    return when (kind) {
        ListKind.Today -> QuestIcons.Today to colors.accent
        ListKind.Inbox -> QuestIcons.Inbox to colors.iconInbox
        ListKind.Logbook -> QuestIcons.Logbook to colors.iconLogbook
        is ListKind.Quest -> null
    }
}

@Composable
private fun EmptyList(kind: ListKind) {
    val text = stringResource(if (kind == ListKind.Inbox) R.string.list_inbox_zero else R.string.list_empty)
    Box(Modifier.fillMaxWidth().padding(vertical = 32.dp), contentAlignment = Alignment.Center) {
        Text(text, style = QuestType.body, color = QuestTheme.colors.textMuted)
    }
}

@Composable
private fun Note(text: String, modifier: Modifier) {
    Box(modifier.fillMaxSize().padding(QuestSpacing.screen), contentAlignment = Alignment.Center) {
        Text(text, style = QuestType.body, color = QuestTheme.colors.textMuted)
    }
}
