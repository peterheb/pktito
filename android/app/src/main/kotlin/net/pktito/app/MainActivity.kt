package net.pktito.app

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp

/**
 * The screen: open a recording, watch its transcript come, share it. The
 * system's picker grants the one document picked, so the app asks for no
 * access to files; the work is TranscribeService's.
 */
class MainActivity : ComponentActivity() {
    private val open = registerForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        if (uri != null) {
            startForegroundService(
                Intent(this, TranscribeService::class.java)
                    .setData(uri)
                    .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            )
            // the progress notification, from Android 13, is the person's to allow
            if (Build.VERSION.SDK_INT >= 33 &&
                checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
            )
                notify.launch(Manifest.permission.POST_NOTIFICATIONS)
        }
    }
    private val notify = registerForActivityResult(ActivityResultContracts.RequestPermission()) {}

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            val dark = isSystemInDarkTheme()
            val colors = when {
                Build.VERSION.SDK_INT >= 31 && dark -> dynamicDarkColorScheme(this)
                Build.VERSION.SDK_INT >= 31 -> dynamicLightColorScheme(this)
                dark -> darkColorScheme()
                else -> lightColorScheme()
            }
            MaterialTheme(colorScheme = colors) {
                Screen(onOpen = { open.launch(arrayOf("audio/*", "video/*")) }, onShare = ::share)
            }
        }
    }

    private fun share() {
        val t = Transcriber.transcript ?: return
        val text = Engine.format(Transcriber.model(this), t, "txt", Transcriber.name)
        val send = Intent(Intent.ACTION_SEND)
            .setType("text/plain")
            .putExtra(Intent.EXTRA_SUBJECT, Transcriber.name)
            .putExtra(Intent.EXTRA_TEXT, text)
        startActivity(Intent.createChooser(send, null))
    }
}

@Composable
private fun Screen(onOpen: () -> Unit, onShare: () -> Unit) {
    Scaffold { pad ->
        Column(
            Modifier
                .padding(pad)
                .padding(horizontal = 16.dp)
                .fillMaxSize()
        ) {
            Text(
                stringResource(R.string.app_name),
                style = MaterialTheme.typography.headlineMedium,
                modifier = Modifier.padding(top = 16.dp)
            )
            if (Transcriber.about.isNotEmpty())
                Text(Transcriber.about, style = MaterialTheme.typography.bodySmall)
            Row(Modifier.padding(vertical = 12.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Button(onClick = onOpen, enabled = !Transcriber.running) { Text(stringResource(R.string.open)) }
                if (Transcriber.running)
                    OutlinedButton(onClick = { Engine.stop() }) { Text(stringResource(R.string.stop)) }
                else if (Transcriber.transcript != null)
                    OutlinedButton(onClick = onShare) { Text(stringResource(R.string.share)) }
            }
            if (Transcriber.running)
                LinearProgressIndicator(progress = { Transcriber.done.toFloat() }, modifier = Modifier.fillMaxWidth())
            Transcriber.error?.let {
                Text(it, color = MaterialTheme.colorScheme.error, modifier = Modifier.padding(vertical = 8.dp))
            }
            if (Transcriber.name.isEmpty())
                Text(stringResource(R.string.hint), style = MaterialTheme.typography.bodyMedium)
            else
                Text(Transcriber.name, style = MaterialTheme.typography.titleMedium, modifier = Modifier.padding(top = 8.dp))
            SelectionContainer(Modifier.weight(1f)) {
                Text(
                    Transcriber.text,
                    style = MaterialTheme.typography.bodyLarge,
                    modifier = Modifier
                        .verticalScroll(rememberScrollState())
                        .padding(vertical = 8.dp)
                )
            }
        }
    }
}
