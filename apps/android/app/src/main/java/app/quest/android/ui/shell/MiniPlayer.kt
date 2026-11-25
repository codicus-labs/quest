package app.quest.android.ui.shell

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import app.quest.android.R
import app.quest.android.ui.components.Dot
import app.quest.android.ui.components.questColor
import app.quest.android.ui.lists.StopPill
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import uniffi.quest_mobile.PlayerCard
import uniffi.quest_mobile.QuestChip

class PlayerActions(
    val openFocus: () -> Unit,
    val stop: () -> Unit,
    /** Resume, or "Back to" from Organizing: starts this task's timer. */
    val start: (String) -> Unit,
) {
    companion object {
        val None = PlayerActions({}, {}, {})
    }
}

/**
 * Above the tabs: the running task (quest, title, time, Stop), Organizing with "Back to <task>", or
 * the last task to resume. Tapping it opens Focus. Nothing when nothing ran.
 */
@Composable
fun MiniPlayer(card: PlayerCard, actions: PlayerActions, modifier: Modifier = Modifier) {
    when (card) {
        PlayerCard.Empty -> Unit
        is PlayerCard.Running -> Frame(actions.openFocus, modifier) {
            Texts(card.title, card.quest?.name, card.quest)
            StopPill(card.timer, card.title, actions.stop, Modifier.testTag("player-stop"))
        }
        is PlayerCard.Organizing -> Frame(actions.openFocus, modifier) {
            Column(Modifier.weight(1f)) {
                Text(stringResource(R.string.organizing), style = QuestType.bodyMedium, color = QuestTheme.colors.text, maxLines = 1)
                val backTo = card.backToTaskId
                val title = card.backToTitle
                if (backTo != null && title != null) {
                    Text(
                        stringResource(R.string.player_back_to, title),
                        style = QuestType.smallMedium,
                        color = QuestTheme.colors.accent,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.clickable { actions.start(backTo) }.padding(vertical = 2.dp).testTag("player-back-to"),
                    )
                }
            }
            StopPill(card.timer, stringResource(R.string.organizing), actions.stop, Modifier.testTag("player-stop"))
        }
        is PlayerCard.Resume -> Frame(actions.openFocus, modifier) {
            Texts(card.title, card.quest?.name, card.quest)
            IconButton(onClick = { actions.start(card.taskId) }, modifier = Modifier.testTag("player-resume")) {
                Icon(Icons.Filled.PlayArrow, contentDescription = stringResource(R.string.focus_resume), tint = QuestTheme.colors.text)
            }
        }
    }
}

@Composable
private fun Frame(onClick: () -> Unit, modifier: Modifier, content: @Composable androidx.compose.foundation.layout.RowScope.() -> Unit) {
    val colors = QuestTheme.colors
    val shape = RoundedCornerShape(QuestSpacing.cardRadius)
    Row(
        modifier
            .padding(horizontal = QuestSpacing.small, vertical = QuestSpacing.tight)
            .widthIn(max = 560.dp)
            .fillMaxWidth()
            .heightIn(min = 56.dp)
            .clip(shape)
            .background(colors.cardRaised)
            .border(QuestSpacing.hairline, colors.border, shape)
            .clickable(onClickLabel = stringResource(R.string.focus_open), onClick = onClick)
            .padding(start = QuestSpacing.medium, end = QuestSpacing.tight)
            .testTag("player"),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(QuestSpacing.medium),
        content = content,
    )
}

@Composable
private fun androidx.compose.foundation.layout.RowScope.Texts(title: String, subtitle: String?, quest: QuestChip?) {
    val colors = QuestTheme.colors
    quest?.let { Dot(questColor(it.colorHex), 8.dp) }
    Column(Modifier.weight(1f)) {
        Text(title, style = QuestType.bodyMedium, color = colors.text, maxLines = 1, overflow = TextOverflow.Ellipsis)
        subtitle?.let { Text(it, style = QuestType.small, color = colors.textMuted, maxLines = 1, overflow = TextOverflow.Ellipsis) }
    }
}
