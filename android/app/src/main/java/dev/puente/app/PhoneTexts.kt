package dev.puente.app

/** Los pocos textos que muestra Android fuera de la interfaz en Rust, en el mismo idioma. */
internal object PhoneTexts {
    private fun pick(spanish: String, english: String, portuguese: String) =
        when (Native.language()) {
            "es" -> spanish
            "pt" -> portuguese
            else -> english
        }

    val channel get() = pick("Pantalla compartida", "Screen sharing", "Tela compartilhada")
    val sharingTitle
        get() = pick(
            "Puente está compartiendo tu pantalla",
            "Puente is sharing your screen",
            "O Puente está compartilhando sua tela",
        )
    val stop get() = pick("Detener", "Stop", "Parar")
    val sending get() = pick("Enviando a la PC…", "Sending to the PC…", "Enviando ao PC…")
    val sent get() = pick("Enviado a la PC", "Sent to the PC", "Enviado ao PC")
    val sendFailed
        get() = pick(
            "No se pudo enviar. ¿Está conectado Puente?",
            "Could not send. Is Puente connected?",
            "Não foi possível enviar. O Puente está conectado?",
        )
    val unknownNumber get() = pick("Número desconocido", "Unknown number", "Número desconhecido")
}
