package app.quest.android.ui.archive

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.onClick
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.unit.dp
import app.quest.android.R
import app.quest.android.ui.components.Dot
import app.quest.android.ui.components.QuestIcons
import app.quest.android.ui.components.questColor
import app.quest.android.ui.task.Caps
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import uniffi.quest_mobile.ArchiveData
import uniffi.quest_mobile.ArchiveItem
import uniffi.quest_mobile.TrashKind

class ArchiveActions(
    val restore: (TrashKind, String) -> Unit,
    /** An archived quest becomes active again. */
    val unarchive: (questId: String) -> Unit,
    /** Opens an archived quest's page. */
    val openQuest: (questId: String) -> Unit,
) {
    companion object {
        /** Does nothing; for screenshots. */
        val None = ArchiveActions({ _, _ -> }, {}, {})
    }
}

/**
 * As on the desktop: quests an older version archived, each with Open and Unarchive, then deleted
 * quests, tasks, checklist items and sessions in the core's groups, each with Restore.
 * Centred and capped in width on tablets.
 */
@Composable
fun ArchiveScreen(state: ArchiveUiState, actions: ArchiveActions) {
    when (state) {
        ArchiveUiState.Loading -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { CircularProgressIndicator() }
        is ArchiveUiState.Failed -> Box(Modifier.fillMaxSize().padding(QuestSpacing.screen), contentAlignment = Alignment.Center) {
            Text(state.message, style = QuestType.body, color = QuestTheme.colors.textMuted)
        }
        is ArchiveUiState.Ready -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.TopCenter) {
            ArchiveList(state.archive, actions)
        }
    }
}

@Composable
private fun ArchiveList(archive: ArchiveData, actions: ArchiveActions) {
    val colors = QuestTheme.colors
    LazyColumn(
        Modifier.widthIn(max = 720.dp).fillMaxSize().testTag("archive"),
        contentPadding = PaddingValues(QuestSpacing.screen),
        verticalArrangement = Arrangement.spacedBy(QuestSpacing.listGap),
    ) {
        item { Text(stringResource(R.string.archive_subtitle), style = QuestType.body, color = colors.textMuted) }
        if (archive.archivedQuests.isNotEmpty()) {
            item(key = "group:archived") {
                Caps(stringResource(R.string.archive_archived_quests), Modifier.padding(top = QuestSpacing.large).semantics { heading() })
            }
            items(archive.archivedQuests, key = { "archived:${it.id}" }) { quest ->
                ArchiveRow(quest) {
                    RowButton(stringResource(R.string.archive_open), stringResource(R.string.archive_open_item, quest.title), "archive-open:${quest.title}") {
                        actions.openQuest(quest.id)
                    }
                    RowButton(stringResource(R.string.archive_unarchive), stringResource(R.string.archive_unarchive_item, quest.title), "archive-unarchive:${quest.title}") {
                        actions.unarchive(quest.id)
                    }
                }
            }
        }
        if (archive.groups.isEmpty() && archive.archivedQuests.isEmpty()) {
            item {
                Text(
                    stringResource(R.string.archive_empty),
                    style = QuestType.body,
                    color = colors.textFaint,
                    modifier = Modifier.padding(top = QuestSpacing.section).testTag("archive-empty"),
                )
            }
        }
        archive.groups.forEach { group ->
            item(key = "group:${group.title}") {
                Caps(group.title, Modifier.padding(top = QuestSpacing.large).semantics { heading() })
            }
            items(group.items, key = { "${it.kind}:${it.id}" }) { item ->
                ArchiveRow(item) {
                    RowButton(stringResource(R.string.archive_restore), stringResource(R.string.archive_restore_item, item.title), "archive-restore:${item.title}", QuestIcons.Restore) {
                        actions.restore(item.kind, item.id)
                    }
                }
            }
        }
    }
}

/** A row: colour, title and meta, then its [buttons]. */
@Composable
private fun ArchiveRow(item: ArchiveItem, buttons: @Composable () -> Unit) {
    val colors = QuestTheme.colors
    val shape = RoundedCornerShape(QuestSpacing.cardRadius)
    Row(
        Modifier
            .fillMaxWidth()
            .background(colors.card, shape)
            .border(QuestSpacing.hairline, colors.border, shape)
            .padding(start = QuestSpacing.large, end = QuestSpacing.small, top = QuestSpacing.small, bottom = QuestSpacing.small)
            .testTag("archive-item:${item.title}"),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(QuestSpacing.medium),
    ) {
        item.colorHex?.let { Dot(questColor(it), QuestSpacing.small) }
        Column(Modifier.weight(1f).semantics(mergeDescendants = true) {}) {
            Text(item.title, style = QuestType.bodyMedium, color = colors.text)
            Text(item.meta, style = QuestType.small, color = colors.textMuted)
        }
        buttons()
    }
}

/**
 * One announcement, "[a11yLabel]" ("Restore <title>"): the button's own text would add a second word.
 * Clearing also drops the button's click from semantics, so it comes back here.
 */
@Composable
private fun RowButton(label: String, a11yLabel: String, tag: String, icon: ImageVector? = null, onClick: () -> Unit) {
    OutlinedButton(
        onClick = onClick,
        contentPadding = PaddingValues(horizontal = QuestSpacing.medium),
        modifier = Modifier.testTag(tag).clearAndSetSemantics {
            contentDescription = a11yLabel
            role = Role.Button
            onClick {
                onClick()
                true
            }
        },
    ) {
        icon?.let { Icon(it, contentDescription = null, modifier = Modifier.padding(end = QuestSpacing.tight)) }
        Text(label, style = QuestType.bodyMedium)
    }
}
