package dev.paimon.probe;

interface IProbeInput {
    int uid() = 0;
    void begin() = 1;
    int tap(int x, int y, String expectedPackage) = 2;
    void cancel() = 3;
    String foregroundPackage() = 4;
    void destroy() = 16777114;
}
