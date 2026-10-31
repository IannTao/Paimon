package dev.paimon.probe

/** This read-only Binder surface deliberately exposes no input operation. */
class LocalizationScreenService : ILocalizationScreen.Stub() {
    private val screenshots = ShellMiniMapCapture()
    override fun capture(): ByteArray = screenshots.capture()
    override fun cancelCapture() = screenshots.cancel()
    override fun destroy() { screenshots.close(); kotlin.system.exitProcess(0) }
}
