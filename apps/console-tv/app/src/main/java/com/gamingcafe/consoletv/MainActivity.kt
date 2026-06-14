package com.gamingcafe.consoletv

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
        setContent {
            ConsoleTvTheme {
                Surface(modifier = Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background) {
                    ConsoleTvAppContent(viewModel)
                }
            }
        }
    }
}

@Composable
private fun ConsoleTvAppContent(viewModel: ConsoleTvViewModel) {
    val state by viewModel.uiState.collectAsState()
    ConsoleTvRoot(state = state, viewModel = viewModel)
}
