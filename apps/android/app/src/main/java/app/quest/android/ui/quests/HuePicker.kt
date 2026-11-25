package app.quest.android.ui.quests

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import app.quest.android.ui.components.questColor
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import uniffi.quest_mobile.HueOption
import uniffi.quest_mobile.QuestHue

/** The nine colours as a radio group of circles; each a 48 dp target named by its colour. */
@Composable
internal fun HuePicker(hues: List<HueOption>, onPick: (QuestHue) -> Unit, tag: String = "quest-hues") {
    val colors = QuestTheme.colors
    FlowRow(Modifier.selectableGroup().testTag(tag)) {
        hues.forEach { option ->
            Box(
                Modifier
                    .size(QuestSpacing.touchTarget)
                    .selectable(selected = option.selected, role = Role.RadioButton, onClick = { onPick(option.hue) })
                    .semantics { contentDescription = option.name }
                    .testTag("quest-hue:${option.name}"),
                contentAlignment = Alignment.Center,
            ) {
                Box(
                    Modifier
                        .size(32.dp)
                        .then(if (option.selected) Modifier.border(2.dp, colors.accent, CircleShape) else Modifier)
                        .padding(if (option.selected) 4.dp else 0.dp)
                        .background(questColor(option.colorHex), CircleShape),
                )
            }
        }
    }
}
