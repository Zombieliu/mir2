using System;

// Exact expressions from Crystal MapObject.PercentHealth/BroadcastHealthChange
// and HeroObject.PercentMana. This program is independent of the Rust helper.
class SourceHealthVectors
{
    static byte Percent(int health, int maxHealth)
    {
        return (byte)(health / (float)maxHealth * 100);
    }

    static byte Expire(long revTime, long now)
    {
        return Math.Min(byte.MaxValue, (byte)Math.Max(5, (revTime - now) / 1000));
    }

    static void Main()
    {
        int[,] pools = { {0, 100}, {1, 1000}, {1, 100}, {53, 100}, {59, 100},
            {99, 100}, {100, 100}, {int.MaxValue - 1, int.MaxValue},
            {1, int.MaxValue}, {123456789, 987654321} };
        for (int i = 0; i < pools.GetLength(0); ++i)
            Console.WriteLine("percent,{0},{1},{2}", pools[i, 0], pools[i, 1],
                Percent(pools[i, 0], pools[i, 1]));
        long[] deadlines = {0, 999, 4999, 5000, 255999, 256000, 260000, 261000};
        foreach (long revTime in deadlines)
            Console.WriteLine("expire,{0},0,{1}", revTime, Expire(revTime, 0));
        Console.WriteLine("expire,1000,2000,{0}", Expire(1000, 2000));
    }
}
