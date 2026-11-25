package app.quest.android.ui.search

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.input.TextFieldLineLimits
import androidx.compose.foundation.text.input.clearText
import androidx.compose.foundation.text.input.TextFieldState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.ArrowForward
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.key
import androidx.compose.runtime.remember
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import app.quest.android.R
import app.quest.android.ui.components.QuestIcons
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import uniffi.quest_mobile.SearchKind
import uniffi.quest_mobile.SearchResult
import uniffi.quest_mobile.SearchToken

class SearchActions(
    val close: () -> Unit,
    val editQuery: (String) -> Unit,
    val run: (SearchToken) -> Unit,
    val runFirst: () -> Unit,
    /** "Capture with AI" with the query as its start text; null hides it (AI capture off or not set up). */
    val captureWithAi: ((query: String) -> Unit)? = null,
) {
    companion object {
        /** Does nothing; for screenshots. */
        val None = SearchActions({}, {}, {}, {})
    }
}

/**
 * Full-screen search (desktop command palette): the field takes focus, the core lists jumps,
 * actions, tasks and quests; the keyboard's Search runs the first one.
 */
@Composable
fun SearchScreen(state: SearchUiState, actions: SearchActions) {
    val colors = QuestTheme.colors
    Column(
        Modifier.fillMaxSize().background(colors.bg).windowInsetsPadding(WindowInsets.safeDrawing).testTag("search"),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Column(Modifier.widthIn(max = 720.dp).fillMaxWidth()) {
            // Until the query is known, the empty field must not report "" over the saved one.
            val withoutTyping = SearchActions(actions.close, editQuery = {}, actions.run, actions.runFirst, actions.captureWithAi)
            when (state) {
                SearchUiState.Loading -> {
                    SearchBar(query = "", mode = SavedSearchMode.Commands, placeholder = stringResource(R.string.search_placeholder), withoutTyping)
                    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { CircularProgressIndicator() }
                }
                is SearchUiState.Failed -> {
                    SearchBar(query = "", mode = SavedSearchMode.Commands, placeholder = stringResource(R.string.search_placeholder), withoutTyping)
                    Text(state.message, style = QuestType.body, color = colors.textMuted, modifier = Modifier.padding(QuestSpacing.screen))
                }
                is SearchUiState.Ready -> {
                    SearchBar(state.query, state.mode, state.placeholder, actions)
                    val capture = actions.captureWithAi
                    if (capture != null && state.mode == SavedSearchMode.Commands) CaptureRow { capture(state.query) }
                    Results(state.results, actions)
                }
            }
        }
    }
}

@Composable
private fun SearchBar(query: String, mode: SavedSearchMode, placeholder: String, actions: SearchActions) {
    val colors = QuestTheme.colors
    Row(Modifier.padding(end = QuestSpacing.large, top = QuestSpacing.small), verticalAlignment = Alignment.CenterVertically) {
        IconButton(onClick = actions.close, modifier = Modifier.testTag("back")) {
            Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = stringResource(R.string.back), tint = colors.textMuted)
        }
        // A new mode (a choice was made) starts a new, empty field.
        key(mode) {
            val focus = remember { FocusRequester() }
            LaunchedEffect(Unit) { focus.requestFocus() }
            // The field owns what is typed and reports it; mirroring the ViewModel back would drop fast typing.
            // Not saveable: the ViewModel keeps the query (also across process death), and a saved
            // field would bring back an old query when search opens again.
            val text = remember { TextFieldState(query) }
            LaunchedEffect(text) { snapshotFlow { text.text.toString() }.collect(actions.editQuery) }
            OutlinedTextField(
                state = text,
                placeholder = { Text(placeholder, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                leadingIcon = { Icon(Icons.Filled.Search, contentDescription = null, tint = colors.textFaint) },
                trailingIcon = {
                    if (text.text.isNotEmpty()) {
                        IconButton(onClick = { text.clearText() }, modifier = Modifier.testTag("search-clear")) {
                            Icon(Icons.Filled.Close, contentDescription = stringResource(R.string.search_clear), tint = colors.textMuted)
                        }
                    }
                },
                lineLimits = TextFieldLineLimits.SingleLine,
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                onKeyboardAction = { actions.runFirst() },
                modifier = Modifier.fillMaxWidth().focusRequester(focus).testTag("search-query"),
            )
        }
    }
}

@Composable
private fun Results(results: List<SearchResult>, actions: SearchActions) {
    val colors = QuestTheme.colors
    if (results.isEmpty()) {
        Text(
            stringResource(R.string.search_no_results),
            style = QuestType.body,
            color = colors.textFaint,
            modifier = Modifier.padding(QuestSpacing.screen).testTag("search-empty"),
        )
        return
    }
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(vertical = QuestSpacing.small)) {
        itemsIndexed(results, key = { index, result -> "$index:${result.title}" }) { _, result -> ResultRow(result) { actions.run(result.token) } }
    }
}

/** Opens AI capture with what is typed (not a core result: it leaves the app's data alone until confirmed). */
@Composable
private fun CaptureRow(onClick: () -> Unit) {
    val colors = QuestTheme.colors
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = QuestSpacing.touchTarget)
            .clickable(role = Role.Button, onClick = onClick)
            .padding(horizontal = QuestSpacing.screen, vertical = QuestSpacing.small)
            .testTag("search-capture"),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(QuestSpacing.medium),
    ) {
        Icon(QuestIcons.Sparkle, contentDescription = null, tint = colors.accent)
        Text(stringResource(R.string.search_capture), style = QuestType.body, color = colors.text, modifier = Modifier.weight(1f))
    }
}

@Composable
private fun ResultRow(result: SearchResult, onClick: () -> Unit) {
    val colors = QuestTheme.colors
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = QuestSpacing.touchTarget)
            .clickable(role = Role.Button, onClick = onClick)
            .padding(horizontal = QuestSpacing.screen, vertical = QuestSpacing.small)
            .testTag("search-result:${result.title}"),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(QuestSpacing.medium),
    ) {
        Icon(result.kind.icon(), contentDescription = null, tint = if (result.kind == SearchKind.CREATE) colors.accent else colors.textMuted)
        Text(result.title, style = QuestType.body, color = colors.text, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
        if (result.hint.isNotEmpty()) Text(result.hint, style = QuestType.small, color = colors.textFaint, maxLines = 1)
    }
}

private fun SearchKind.icon(): ImageVector = when (this) {
    SearchKind.JUMP -> Icons.AutoMirrored.Filled.ArrowForward
    SearchKind.ACTION -> Icons.Filled.PlayArrow
    SearchKind.TASK -> Icons.Filled.CheckCircle
    SearchKind.QUEST -> QuestIcons.Trophy
    SearchKind.CREATE -> Icons.Filled.Add
    SearchKind.CHOICE -> Icons.AutoMirrored.Filled.KeyboardArrowRight
}
