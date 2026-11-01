package dev.paimon.probe;

interface INavigationInput {
    void begin(long runId) = 1;
    byte[] capture(long runId) = 2;
    long[] execute(long runId, long serial, int kind, int dx, int durationMs, long capturedAt, int tapX, int tapY) = 3;
    boolean cancel(long runId) = 4;
    long holdingSince(long runId) = 5;
    String foregroundPackage() = 6;
    byte[] captureScreen(long runId) = 7;
    void destroy() = 16777114;
}
