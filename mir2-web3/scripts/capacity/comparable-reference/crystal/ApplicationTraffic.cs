namespace ComparableCrystal;

internal sealed record ApplicationCounterSnapshot(int ActorOrdinal, int Connection, double ReadStartedAtMs,
    double CounterAtMs, long BytesSent, long BytesReceived);
internal sealed record ApplicationCounterWindow(int ActorOrdinal, int Connection, ApplicationCounterSnapshot Start,
    ApplicationCounterSnapshot End, long BytesSent, long BytesReceived);

internal static class ApplicationTraffic
{
    public const string Scope = "native application frames including 4-byte frame header; excludes TCP/IP and link overhead";
    public const string Semantics = "cumulative completed WriteAsync bytes and complete frame reads; in-flight partial operations are not counted until completion; each counter read is bracketed by readStartedAtMs/counterAtMs";
    public static ApplicationCounterWindow Window(ApplicationCounterSnapshot start, ApplicationCounterSnapshot end)
    {
        Guard.Need(start.ActorOrdinal == end.ActorOrdinal && start.Connection == end.Connection, "traffic_window_connection_mismatch");
        Guard.Need(start.CounterAtMs <= end.ReadStartedAtMs && end.BytesSent >= start.BytesSent && end.BytesReceived >= start.BytesReceived,
            "traffic_counter_or_time_regressed");
        return new(start.ActorOrdinal, start.Connection, start, end, end.BytesSent - start.BytesSent, end.BytesReceived - start.BytesReceived);
    }
    public static object Steady(ApplicationCounterSnapshot[] start, ApplicationCounterSnapshot[] end, int population, double? anchor, int durationMs, bool durationReached)
    {
        var perActor = start.Join(end, x => (x.ActorOrdinal, x.Connection), x => (x.ActorOrdinal, x.Connection), Window).ToArray();
        return new { scope = Scope, counterSemantics = Semantics, startCapture = "after measurement anchor wait, before first planned action send",
            endCapture = "after duration wait and before original-deadline observation tail or any cleanup; aborted measurements captured before cleanup",
            includesSteadyKeepAlive = true, bootstrapEquipmentAndCleanupExcluded = true, measurementAnchorMs = anchor,
            plannedMeasurementEndMs = anchor is null ? (double?)null : anchor + durationMs, durationReached,
            samplingComplete = start.Length == population && end.Length == population && perActor.Length == population,
            perActor, bytesSent = perActor.Sum(x => x.BytesSent), bytesReceived = perActor.Sum(x => x.BytesReceived) };
    }
}
