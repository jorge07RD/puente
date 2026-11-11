package dev.puente.app

/**
 * Calidad del vídeo que pide la PC. Vive solo en memoria: cada PC nueva empieza en la
 * normal, que es la que usan los móviles que no conocen la orden.
 */
internal object CaptureQuality {
    /** `adaptive`: el codificador mueve el bitrate según aguante la red. */
    data class Level(val maxDimension: Int, val bitRate: Int, val frameRate: Int, val adaptive: Boolean = false)

    private val AUTO = Level(1920, 8_000_000, 60, adaptive = true)
    private val SAVER = Level(1280, 4_000_000, 30)
    private val BALANCED = Level(1920, 8_000_000, 60)
    private val HIGH = Level(2560, 16_000_000, 60)

    /**
     * El último bitrate que encontró el modo automático. Sobrevive a los reinicios del
     * codificador al girar, para no volver a empezar a tientas.
     */
    @Volatile
    var autoBitRate: Int = AUTO.bitRate

    @Volatile
    var current: Level = BALANCED
        private set

    /** Devuelve si cambió, para no reiniciar el codificador sin motivo. */
    fun set(name: String): Boolean {
        val level = when (name) {
            "auto" -> AUTO
            "saver" -> SAVER
            "high" -> HIGH
            else -> BALANCED
        }
        if (level == current) return false
        current = level
        return true
    }
}
