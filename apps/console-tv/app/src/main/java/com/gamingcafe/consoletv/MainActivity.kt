package com.gamingcafe.consoletv

import android.net.Uri
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.viewModels
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import com.gamingcafe.consoletv.ui.ConsoleTvRoot
import com.gamingcafe.consoletv.ui.ConsoleTvTheme

class MainActivity : ComponentActivity() {
    private val viewModel: ConsoleTvViewModel by viewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        handleDeepLink(intent?.data)
        setContent {
            ConsoleTvTheme {
                Surface(modifier = Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background) {
                    ConsoleTvAppContent(viewModel)
                }
            }
        }
    }

    override fun onNewIntent(intent: android.content.Intent) {
        super.onNewIntent(intent)
        handleDeepLink(intent.data)
    }

    private fun handleDeepLink(uri: Uri?) {
        if (uri == null) return
        if (uri.scheme == "gamingcafe" && uri.host == "tv") {
            val token = uri.getQueryParameter("token")
            if (!token.isNullOrBlank()) {
                viewModel.handleDeepLinkSsoToken(token)
            }
        }
    }
}

@Composable
private fun ConsoleTvAppContent(viewModel: ConsoleTvViewModel) {
    val state by viewModel.uiState.collectAsState()
    ConsoleTvRoot(state = state, viewModel = viewModel)
}
