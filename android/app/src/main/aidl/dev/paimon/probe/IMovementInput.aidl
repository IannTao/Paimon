package dev.paimon.probe;

interface IMovementInput {
    void begin(long runId, String expectedPackage) = 0;
    byte[] capture(long runId) = 1;
    boolean move(long runId) = 2;
    long cancel(long runId) = 3;
    String foregroundPackage() = 4;
    long holdingSince(long runId) = 5;
    void destroy() = 16777114;
}
