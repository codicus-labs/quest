package app.quest.android.ui.navigation

import app.quest.android.ui.components.CappedFontScale
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.heading
import androidx.annotation.StringRes
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.ui.Alignment
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import app.quest.android.ui.shell.Avatar
import app.quest.android.ui.theme.QuestSpacing
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.consumeWindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.ime
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.adaptive.navigationsuite.NavigationSuiteDefaults
import androidx.compose.material3.adaptive.navigationsuite.NavigationSuiteItemColors
import androidx.compose.material3.adaptive.navigationsuite.NavigationSuiteScaffold
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.layout.boundsInRoot
import androidx.compose.ui.layout.findRootCoordinates
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import app.quest.android.R
import app.quest.android.ui.components.QuestIcons
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType

/** Five labels share the bar; beyond this text size they would break mid-word. */
private const val NAV_LABEL_MAX_FONT_SCALE = 1.3f

/** The four places in the bottom bar (phones) or navigation rail (tablets); Account opens a drawer. */
enum class Tab(@StringRes val label: Int, val icon: ImageVector) {
    Today(R.string.nav_today, QuestIcons.Today),
    Inbox(R.string.nav_inbox, QuestIcons.Inbox),
    Quests(R.string.nav_quests, QuestIcons.Flag),
    Tracked(R.string.nav_tracked, QuestIcons.Focus),
}

/**
 * The app frame: bottom bar or navigation rail chosen from the window size class (compact → bar,
 * medium/expanded → rail) with the four tabs and Account (the workspace's avatar and name). Above
 * the content a slim row with [leading] (Today's segment, a quest's way back) and Search; below it
 * the note while the timer runs in another workspace. The [player] sits above the tabs.
 */
@Composable
fun QuestScaffold(
    tab: Tab,
    onTab: (Tab) -> Unit,
    onSearch: () -> Unit,
    onAccount: () -> Unit,
    /** The shown workspace; empty while it is still opening. */
    workspaceName: String,
    leading: @Composable () -> Unit = {},
    timerNote: @Composable () -> Unit = {},
    player: @Composable () -> Unit = {},
    snackbarHost: @Composable () -> Unit = {},
    content: @Composable () -> Unit,
) {
    val colors = QuestTheme.colors
    val itemColors = itemColors()
    val label = @Composable { text: String -> CappedFontScale(NAV_LABEL_MAX_FONT_SCALE) { Text(text, maxLines = 1, overflow = TextOverflow.Ellipsis) } }
    NavigationSuiteScaffold(
        navigationSuiteItems = {
            Tab.entries.forEach { entry ->
                item(
                    selected = entry == tab,
                    onClick = { onTab(entry) },
                    icon = { Icon(entry.icon, contentDescription = null) },
                    label = { label(stringResource(entry.label)) },
                    modifier = Modifier.testTag("tab-${entry.name.lowercase()}"),
                    colors = itemColors,
                )
            }
            item(
                selected = false,
                onClick = onAccount,
                icon = { Avatar(workspaceName, 24.dp) },
                label = { label(workspaceName.ifEmpty { stringResource(R.string.account) }) },
                modifier = Modifier.testTag("tab-account"),
                colors = itemColors,
            )
        },
        navigationSuiteColors = NavigationSuiteDefaults.colors(
            navigationBarContainerColor = colors.bar,
            navigationRailContainerColor = colors.bar,
        ),
        containerColor = colors.bg,
    ) {
        Scaffold(
            containerColor = colors.bg,
            snackbarHost = snackbarHost,
            topBar = {
                Column(Modifier.statusBarsPadding()) {
                    Row(Modifier.fillMaxWidth().heightIn(min = 48.dp).padding(start = QuestSpacing.medium, end = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                        Box(Modifier.weight(1f)) { leading() }
                        IconButton(onClick = onSearch, modifier = Modifier.testTag("open-search")) {
                            Icon(Icons.Filled.Search, contentDescription = stringResource(R.string.search), tint = colors.textMuted)
                        }
                    }
                    timerNote()
                }
            },
            bottomBar = player,
        ) { padding ->
            KeyboardAwareBox(Modifier.fillMaxSize().padding(padding).consumeWindowInsets(padding)) { content() }
        }
    }
}

/**
 * Edge-to-edge does not resize the window for the keyboard, so this box pads its bottom by the
 * part of the keyboard that overlaps it. What lies below the box (bottom bar, system bar) already
 * covers the rest; plain `imePadding` would count the bottom bar twice.
 */
@Composable
private fun KeyboardAwareBox(modifier: Modifier, content: @Composable () -> Unit) {
    val density = LocalDensity.current
    val keyboardHeight = WindowInsets.ime.getBottom(density)
    var heightBelowBox by remember { mutableIntStateOf(0) }
    val overlap = (keyboardHeight - heightBelowBox).coerceAtLeast(0)
    Box(
        modifier
            .onGloballyPositioned { box ->
                heightBelowBox = box.findRootCoordinates().size.height - box.boundsInRoot().bottom.toInt()
            }
            .padding(bottom = with(density) { overlap.toDp() }),
    ) { content() }
}

@Composable
private fun itemColors(): NavigationSuiteItemColors {
    val colors = QuestTheme.colors
    return NavigationSuiteDefaults.itemColors(
        navigationBarItemColors = androidx.compose.material3.NavigationBarItemDefaults.colors(
            selectedIconColor = colors.accent,
            selectedTextColor = colors.text,
            indicatorColor = colors.selected,
            unselectedIconColor = colors.textMuted,
            unselectedTextColor = colors.textMuted,
        ),
        navigationRailItemColors = androidx.compose.material3.NavigationRailItemDefaults.colors(
            selectedIconColor = colors.accent,
            selectedTextColor = colors.text,
            indicatorColor = colors.selected,
            unselectedIconColor = colors.textMuted,
            unselectedTextColor = colors.textMuted,
        ),
    )
}

/** A screen opened from the top bar (Search, Account, Settings), with a back arrow. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DetailScaffold(title: String, onBack: () -> Unit, snackbarHost: @Composable () -> Unit = {}, content: @Composable () -> Unit) {
    val colors = QuestTheme.colors
    Scaffold(
        containerColor = colors.bg,
        snackbarHost = snackbarHost,
        topBar = {
            TopAppBar(
                title = { Text(title, style = QuestType.stat, modifier = Modifier.semantics { heading() }) },
                navigationIcon = {
                    IconButton(onClick = onBack, modifier = Modifier.testTag("back")) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = stringResource(R.string.back))
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = colors.bg,
                    navigationIconContentColor = colors.textMuted,
                    titleContentColor = colors.text,
                ),
            )
        },
    ) { padding ->
        Box(Modifier.fillMaxSize().padding(padding)) { content() }
    }
}
