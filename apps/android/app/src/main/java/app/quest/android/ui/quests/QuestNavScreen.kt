package app.quest.android.ui.quests

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.wrapContentWidth
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import app.quest.android.R
import app.quest.android.ui.components.HoursRing
import app.quest.android.ui.components.QuestIcons
import app.quest.android.ui.components.questColor
import app.quest.android.ui.lists.ListMaxWidth
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import uniffi.quest_mobile.QuestNavRow

class QuestNavActions(
    val openQuest: (String) -> Unit,
    val newQuest: () -> Unit,
    val openLogbook: () -> Unit,
)

/** The Quests tab: active quests with this week's hours, the Logbook, then completed quests. */
@Composable
fun QuestNavScreen(state: QuestNavUiState, actions: QuestNavActions, modifier: Modifier = Modifier, top: @Composable () -> Unit = {}) {
    when (state) {
        QuestNavUiState.Loading -> Box(modifier.fillMaxSize(), contentAlignment = Alignment.Center) { CircularProgressIndicator() }
        is QuestNavUiState.Failed -> Box(modifier.fillMaxSize().padding(QuestSpacing.screen), contentAlignment = Alignment.Center) {
            Text(state.message, style = QuestType.body, color = QuestTheme.colors.textMuted)
        }
        is QuestNavUiState.Ready -> LazyColumn(
            modifier.fillMaxSize().wrapContentWidth(Alignment.CenterHorizontally).widthIn(max = ListMaxWidth).testTag("quests-list"),
            contentPadding = PaddingValues(start = QuestSpacing.medium, end = QuestSpacing.medium, bottom = 88.dp),
        ) {
            item(key = "top") { top() }
            item(key = "header") { Header(actions.newQuest) }
            if (state.nav.active.isEmpty()) {
                item(key = "empty") { Text(stringResource(R.string.quests_nav_empty), style = QuestType.body, color = QuestTheme.colors.textMuted, modifier = Modifier.padding(QuestSpacing.small)) }
            }
            questRows("active", state.nav.active, actions.openQuest)
            item(key = "logbook") { LogbookRow(actions.openLogbook) }
            if (state.nav.completed.isNotEmpty()) {
                item(key = "completed") {
                    Text(
                        stringResource(R.string.quests_completed),
                        style = QuestType.caps,
                        color = QuestTheme.colors.textMuted,
                        modifier = Modifier.padding(start = QuestSpacing.small, top = QuestSpacing.section, bottom = QuestSpacing.tight).semantics { heading() },
                    )
                }
                questRows("completed", state.nav.completed, actions.openQuest)
            }
        }
    }
}

@Composable
private fun Header(newQuest: () -> Unit) {
    Row(Modifier.fillMaxWidth().padding(start = QuestSpacing.small, top = QuestSpacing.small, bottom = QuestSpacing.small), verticalAlignment = Alignment.CenterVertically) {
        Text(stringResource(R.string.nav_quests), style = QuestType.heading, color = QuestTheme.colors.text, modifier = Modifier.weight(1f).semantics { heading() })
        IconButton(onClick = newQuest, modifier = Modifier.testTag("quests-new")) {
            Icon(Icons.Filled.Add, contentDescription = stringResource(R.string.quests_new), tint = QuestTheme.colors.textMuted)
        }
    }
}

private fun LazyListScope.questRows(key: String, rows: List<QuestNavRow>, open: (String) -> Unit) {
    items(rows, key = { "$key-${it.id}" }) { row -> QuestRow(row, open) }
}

@Composable
private fun QuestRow(row: QuestNavRow, open: (String) -> Unit) {
    val colors = QuestTheme.colors
    NavRow(onClick = { open(row.id) }, label = row.a11yLabel, tag = "quest-row:${row.title}") {
        HoursRing(row.ring, questColor(row.colorHex))
        Text(row.title, style = QuestType.task, color = colors.text, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
        Text(row.weekHours, style = QuestType.monoTime, color = colors.textMuted)
    }
}

@Composable
private fun LogbookRow(open: () -> Unit) {
    val label = stringResource(R.string.list_logbook)
    NavRow(onClick = open, label = label, tag = "quests-logbook", modifier = Modifier.padding(top = QuestSpacing.medium)) {
        Icon(QuestIcons.Logbook, contentDescription = null, tint = QuestTheme.colors.iconLogbook, modifier = Modifier.size(22.dp))
        Text(label, style = QuestType.task, color = QuestTheme.colors.text, modifier = Modifier.weight(1f))
    }
}

@Composable
private fun NavRow(onClick: () -> Unit, label: String, tag: String, modifier: Modifier = Modifier, content: @Composable RowScope.() -> Unit) {
    Row(
        modifier
            .fillMaxWidth()
            .heightIn(min = 52.dp)
            .testTag(tag)
            .clickable(onClick = onClick)
            .clearAndSetSemantics {
                contentDescription = label
                role = Role.Button
            }
            .padding(horizontal = QuestSpacing.small),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(QuestSpacing.medium),
    ) { content() }
}
