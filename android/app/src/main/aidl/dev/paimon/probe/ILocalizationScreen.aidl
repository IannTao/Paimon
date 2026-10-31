package dev.paimon.probe;

// Read-only shell boundary: deliberately exposes no game input operation.
interface ILocalizationScreen {
    byte[] capture();
    void cancelCapture();
    void destroy();
}
