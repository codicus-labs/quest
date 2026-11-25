package app.quest.android.ui.navigation

import app.quest.android.ui.components.CappedFontScale
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.heading
import androidx.annotation.StringRes
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.consumeWindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.ime
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.List
import androidx.compose.material.icons.filled.AccountCircle
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.filled.DateRange
import androidx.compose.material.icons.filled.Home
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.Star
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
import app.quest.android.ui.workspaces.SwitcherActions
import app.quest.android.ui.workspaces.TimerElsewhereBar
import app.quest.android.ui.workspaces.WorkspaceSwitcher
import uniffi.quest_mobile.WorkspacesData

/** Five labels share the bar; beyond this text size they would break mid-word. */
private const val NAV_LABEL_MAX_FONT_SCALE = 1.3f

/** The five places in the bottom bar (phones) or navigation rail (tablets). */
enum class TopDestination(@StringRes val label: Int, val icon: ImageVector) {
    Home(R.string.nav_home, Icons.Filled.Home),
    Plan(R.string.nav_plan, Icons.AutoMirrored.Filled.List),
    Calendar(R.string.nav_calendar, Icons.Filled.DateRange),
    Quests(R.string.nav_quests, Icons.Filled.Star),
    Review(R.string.nav_review, Icons.Filled.CheckCircle),
}

/**
 * The app frame: bottom bar or navigation rail chosen from the window size class (compact →
 * bar, medium/expanded → rail), and a top bar with the workspace switcher on the left, Focus, Search
 * and Account on the right, and below it a note while the focus timer runs in another workspace.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun QuestScaffold(
    destination: TopDestination,
    onNavigate: (TopDestination) -> Unit,
    onSearch: () -> Unit,
    onAccount: () -> Unit,
    onFocus: () -> Unit = {},
    snackbarHost: @Composable () -> Unit = {},
    /** The switcher's workspaces; null (still opening) shows the destination's title instead. */
    workspaces: WorkspacesData? = null,
    switcher: SwitcherActions = SwitcherActions.None,
    content: @Composable () -> Unit,
) {
    val colors = QuestTheme.colors
    val itemColors = itemColors()
    NavigationSuiteScaffold(
        navigationSuiteItems = {
            TopDestination.entries.forEach { entry ->
                item(
                    selected = entry == destination,
                    onClick = { onNavigate(entry) },
                    icon = { Icon(entry.icon, contentDescription = null) },
                    label = { CappedFontScale(NAV_LABEL_MAX_FONT_SCALE) { Text(stringResource(entry.label), maxLines = 1) } },
                    modifier = Modifier.testTag("tab-${entry.name.lowercase()}"),
                    colors = itemColors,
                )
            }
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
                Column {
                    TopAppBar(
                        title = {
                            when {
                                workspaces != null -> WorkspaceSwitcher(workspaces, switcher)
                                destination != TopDestination.Home -> Text(stringResource(destination.label), style = QuestType.stat, modifier = Modifier.semantics { heading() })
                            }
                        },
                        actions = {
                            IconButton(onClick = onFocus, modifier = Modifier.testTag("open-focus")) {
                                Icon(QuestIcons.Focus, contentDescription = stringResource(R.string.focus))
                            }
                            IconButton(onClick = onSearch, modifier = Modifier.testTag("open-search")) {
                                Icon(Icons.Filled.Search, contentDescription = stringResource(R.string.search))
                            }
                            IconButton(onClick = onAccount, modifier = Modifier.testTag("open-account")) {
                                Icon(Icons.Filled.AccountCircle, contentDescription = stringResource(R.string.account))
                            }
                        },
                        colors = TopAppBarDefaults.topAppBarColors(
                            containerColor = colors.bg,
                            actionIconContentColor = colors.textMuted,
                            titleContentColor = colors.text,
                        ),
                    )
                    TimerElsewhereBar(workspaces, switcher.showTimer)
                }
            },
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
