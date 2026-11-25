package app.quest.android.ui.shell

import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import app.quest.android.R
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType

/** "Today | Plan" on the Today tab: the day's list, or the plan of the coming days. */
@Composable
fun TodaySegment(showPlan: Boolean, onChange: (showPlan: Boolean) -> Unit, modifier: Modifier = Modifier) {
    val colors = QuestTheme.colors
    val segmentColors = SegmentedButtonDefaults.colors(
        activeContainerColor = colors.selected,
        activeContentColor = colors.text,
        activeBorderColor = colors.borderStrong,
        inactiveContainerColor = colors.bg,
        inactiveContentColor = colors.textMuted,
        inactiveBorderColor = colors.borderStrong,
    )
    SingleChoiceSegmentedButtonRow(modifier.testTag("today-segment")) {
        listOf(false to R.string.nav_today, true to R.string.nav_plan).forEachIndexed { index, (plan, label) ->
            SegmentedButton(
                selected = showPlan == plan,
                onClick = { onChange(plan) },
                shape = SegmentedButtonDefaults.itemShape(index, 2),
                colors = segmentColors,
                icon = {},
                modifier = Modifier.testTag(if (plan) "segment-plan" else "segment-today"),
            ) { Text(stringResource(label), style = QuestType.nav) }
        }
    }
}
