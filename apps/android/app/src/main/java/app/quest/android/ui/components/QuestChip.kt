package app.quest.android.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import app.quest.android.R
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import uniffi.quest_mobile.QuestChip as QuestRef

/** A quest's name with its colour dot, in a pill; "No quest" when [quest] is null. */
@Composable
fun QuestChip(quest: QuestRef?, modifier: Modifier = Modifier) {
    val colors = QuestTheme.colors
    val shape = RoundedCornerShape(50)
    Row(
        modifier = modifier
            .height(QuestSpacing.chipHeight)
            .background(colors.chip, shape)
            .border(QuestSpacing.hairline, colors.border, shape)
            .padding(horizontal = 10.dp),
        horizontalArrangement = Arrangement.spacedBy(QuestSpacing.tight),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Dot(quest?.let { questColor(it.colorHex) } ?: colors.textFaint, 7.dp)
        Text(
            quest?.name ?: stringResource(R.string.no_quest),
            style = QuestType.smallMedium,
            color = colors.textMuted,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

@Composable
fun Dot(color: Color, size: Dp, modifier: Modifier = Modifier) {
    Box(modifier.size(size).background(color, CircleShape))
}

/** A quest accent colour the core sends as "#rrggbb". */
fun questColor(hex: String): Color =
    runCatching { Color(android.graphics.Color.parseColor(hex)) }.getOrDefault(Color.Gray)
