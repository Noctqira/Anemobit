using System.Windows;
using System.Windows.Media;

namespace Anemobit.Controls;

/// <summary>
/// Monotone cubic Hermite curve (Fritsch–Carlson): passes through every point without overshooting neighboring points between them,
/// so the rate chart never shows false dips below 0 or above the peak (same as .monotone in Swift Charts on macOS).
/// </summary>
internal static class MonotoneCurve
{
    /// <param name="context">Geometry to draw into.</param>
    /// <param name="points">Points with increasing x.</param>
    /// <param name="startFigure">Whether to start a new figure.</param>
    /// <param name="closeToBottom">When not null, closes the curve down to this y to form an area.</param>
    public static void Draw(StreamGeometryContext context, IReadOnlyList<Point> points, bool startFigure, double? closeToBottom)
    {
        if (points.Count == 0)
        {
            return;
        }

        var filled = closeToBottom.HasValue;
        if (startFigure)
        {
            var origin = filled ? new Point(points[0].X, closeToBottom!.Value) : points[0];
            context.BeginFigure(origin, isFilled: filled, isClosed: filled);
            if (filled)
            {
                context.LineTo(points[0], isStroked: false, isSmoothJoin: false);
            }
        }

        if (points.Count == 1)
        {
            context.LineTo(new Point(points[0].X + 0.5, points[0].Y), isStroked: true, isSmoothJoin: true);
        }
        else
        {
            var tangents = Tangents(points);
            for (var i = 0; i < points.Count - 1; i++)
            {
                var p0 = points[i];
                var p1 = points[i + 1];
                var dx = (p1.X - p0.X) / 3;
                context.BezierTo(
                    new Point(p0.X + dx, p0.Y + (tangents[i] * dx)),
                    new Point(p1.X - dx, p1.Y - (tangents[i + 1] * dx)),
                    p1,
                    isStroked: true,
                    isSmoothJoin: true);
            }
        }

        if (filled)
        {
            context.LineTo(new Point(points[^1].X, closeToBottom!.Value), isStroked: false, isSmoothJoin: false);
        }
    }

    private static double[] Tangents(IReadOnlyList<Point> points)
    {
        var n = points.Count;
        var slopes = new double[n - 1];
        for (var i = 0; i < n - 1; i++)
        {
            var dx = points[i + 1].X - points[i].X;
            slopes[i] = dx == 0 ? 0 : (points[i + 1].Y - points[i].Y) / dx;
        }

        var tangents = new double[n];
        tangents[0] = slopes[0];
        tangents[n - 1] = slopes[n - 2];
        for (var i = 1; i < n - 1; i++)
        {
            tangents[i] = slopes[i - 1] * slopes[i] <= 0 ? 0 : (slopes[i - 1] + slopes[i]) / 2;
        }

        // Fritsch–Carlson: limit tangent magnitudes to stay monotone.
        for (var i = 0; i < n - 1; i++)
        {
            if (slopes[i] == 0)
            {
                tangents[i] = 0;
                tangents[i + 1] = 0;
                continue;
            }
            var a = tangents[i] / slopes[i];
            var b = tangents[i + 1] / slopes[i];
            var h = (a * a) + (b * b);
            if (h > 9)
            {
                var t = 3 / Math.Sqrt(h);
                tangents[i] = t * a * slopes[i];
                tangents[i + 1] = t * b * slopes[i];
            }
        }
        return tangents;
    }
}
