package dev.carbonhv.app

import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.ServiceConnection
import android.os.Bundle
import android.os.IBinder
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.runtime.mutableStateOf
import dev.carbonhv.host.ICarbonHvHostService

class MainActivity : ComponentActivity() {
    private val hostService = mutableStateOf<ICarbonHvHostService?>(null)
    private var hostServiceBound = false

    private val hostConnection = object : ServiceConnection {
        override fun onServiceConnected(name: ComponentName?, binder: IBinder?) {
            hostService.value = ICarbonHvHostService.Stub.asInterface(binder)
        }

        override fun onServiceDisconnected(name: ComponentName?) {
            hostService.value = null
        }

        override fun onBindingDied(name: ComponentName?) {
            hostService.value = null
            hostServiceBound = false
            bindHostIntegration()
        }

        override fun onNullBinding(name: ComponentName?) {
            hostService.value = null
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        bindHostIntegration()
        setContent {
            CarbonHvApp(hostService = hostService.value)
        }
    }

    override fun onDestroy() {
        if (hostServiceBound) {
            unbindService(hostConnection)
            hostServiceBound = false
        }
        hostService.value = null
        super.onDestroy()
    }

    private fun bindHostIntegration() {
        if (hostServiceBound) return
        val intent = Intent().setComponent(
            ComponentName(HOST_PACKAGE, HOST_SERVICE),
        )
        hostServiceBound = try {
            bindService(intent, hostConnection, Context.BIND_AUTO_CREATE)
        } catch (_: SecurityException) {
            false
        }
    }

    private companion object {
        const val HOST_PACKAGE = "dev.carbonhv.host"
        const val HOST_SERVICE = "dev.carbonhv.host.CarbonHvHostService"
    }
}
