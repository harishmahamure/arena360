package com.gamingcafe.consoletv.data

import com.gamingcafe.consoletv.BuildConfig
import com.google.gson.Gson
import com.google.gson.JsonObject
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import java.util.concurrent.TimeUnit

data class RealtimeEvent(
    val eventType: String,
    val payload: JsonObject?,
)

class RealtimeClient(
    private val onEvent: (RealtimeEvent) -> Unit,
    private val onConnectionChanged: (Boolean) -> Unit,
) {
    private val gson = Gson()
    private val client =
        OkHttpClient.Builder()
            .pingInterval(30, TimeUnit.SECONDS)
            .build()
    private var webSocket: WebSocket? = null
    private var token: String? = null
    private val subscriptions = linkedSetOf<String>()
    private var disposed = false
    private var reconnectAttempt = 0

    fun connect(wsToken: String, channels: List<String>) {
        disposed = false
        token = wsToken
        subscriptions.clear()
        subscriptions.addAll(channels)
        openSocket()
    }

    fun disconnect() {
        disposed = true
        webSocket?.close(1000, "dispose")
        webSocket = null
        onConnectionChanged(false)
    }

    private fun wsUrl(): String = BuildConfig.GATEWAY_URL.trimEnd('/')

    private fun openSocket() {
        val bearer = token ?: return
        val request =
            Request.Builder()
                .url(wsUrl())
                .addHeader("Sec-WebSocket-Protocol", "bearer, $bearer")
                .build()
        webSocket =
            client.newWebSocket(
                request,
                object : WebSocketListener() {
                    override fun onOpen(webSocket: WebSocket, response: Response) {
                        reconnectAttempt = 0
                        onConnectionChanged(true)
                        if (subscriptions.isNotEmpty()) {
                            val frame =
                                gson.toJson(
                                    mapOf("type" to "Subscribe", "channels" to subscriptions.toList()),
                                )
                            webSocket.send(frame)
                        }
                    }

                    override fun onMessage(webSocket: WebSocket, text: String) {
                        val frame = gson.fromJson(text, JsonObject::class.java)
                        if (frame.get("type")?.asString == "Event") {
                            frame.get("msg_id")?.let { id ->
                                webSocket.send(gson.toJson(mapOf("type" to "Ack", "msg_id" to id)))
                            }
                            val eventType = frame.get("event_type")?.asString ?: return
                            onEvent(RealtimeEvent(eventType, frame.getAsJsonObject("payload")))
                        }
                    }

                    override fun onClosed(webSocket: WebSocket, code: Int, reason: String) {
                        onConnectionChanged(false)
                        scheduleReconnect()
                    }

                    override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
                        onConnectionChanged(false)
                        scheduleReconnect()
                    }
                },
            )
    }

    private fun scheduleReconnect() {
        if (disposed || token == null) return
        reconnectAttempt += 1
        val delay = minOf(5_000L, 1_000L * reconnectAttempt)
        Thread {
            Thread.sleep(delay)
            if (!disposed) openSocket()
        }.start()
    }
}
