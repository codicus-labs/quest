package app.quest.android.ui.quests

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.onClick
import androidx.compose.ui.semantics.role
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import app.quest.android.R
import app.quest.android.ui.components.questColor
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import uniffi.quest_mobile.QuestCard

/**
 * A quest on the Quests tab, as on the desktop: its colour, name and open tasks, this week's hours
 * large and the total beside them. One button for TalkBack, read as the core's [QuestCard.a11yLabel];
 * a tap opens the quest's editor.
 */
@Composable
fun QuestCardView(card: QuestCard, onOpen: () -> Unit, modifier: Modifier = Modifier) {
    val colors = QuestTheme.colors
    val shape = RoundedCornerShape(QuestSpacing.cardRadius)
    val editLabel = stringResource(R.string.quest_edit)
    Column(
        modifier
            .fillMaxWidth()
            .clip(shape)
            .background(colors.card)
            .border(QuestSpacing.hairline, colors.border, shape)
            .clickable(onClick = onOpen)
            .testTag("quest-card:${card.title}")
            .clearAndSetSemantics {
                contentDescription = card.a11yLabel
                role = Role.Button
                onClick(editLabel) {
                    onOpen()
                    true
                }
            }
            .padding(QuestSpacing.large),
        verticalArrangement = Arrangement.spacedBy(QuestSpacing.large),
    ) {
        Row(horizontalArrangement = Arrangement.spacedBy(QuestSpacing.medium)) {
            Box(Modifier.size(36.dp).background(colors.chip, RoundedCornerShape(9.dp)), contentAlignment = Alignment.Center) {
                Box(Modifier.size(12.dp).background(questColor(card.colorHex), CircleShape))
            }
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text(card.title, style = QuestType.stat, color = colors.text, maxLines = 2, overflow = TextOverflow.Ellipsis)
                Text(card.openTasks, style = QuestType.small, color = colors.textMuted)
            }
        }
        Row(verticalAlignment = Alignment.Bottom) {
            Column(Modifier.weight(1f)) {
                Text(card.weekHours, style = QuestType.bigStat, color = colors.text, modifier = Modifier.testTag("quest-week:${card.title}"))
                Text(stringResource(R.string.quests_this_week).uppercase(), style = QuestType.caps, color = colors.textMuted)
            }
            Text(card.totalHours, style = QuestType.small, color = colors.textMuted, modifier = Modifier.testTag("quest-total:${card.title}"))
        }
    }
}
