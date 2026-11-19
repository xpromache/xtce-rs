import java.util.*;
public class Gen {
  public static void main(String[] a) {
    Random r = new Random(42);
    StringBuilder sb = new StringBuilder();
    double[] special = {1, 0.1, 0.3, 1e7, 9999999.999, 1e-3, 0.00099999, 1e22, 1e23, 2e23, 5e-324, 1.0E-5, 100, 123456789012345678.0};
    for (double d : special) sb.append("d ").append(Long.toHexString(Double.doubleToRawLongBits(d))).append(' ').append(Double.toString(d)).append('\n');
    for (int i = 0; i < 200000; i++) {
      double d;
      switch (i % 4) {
        case 0: d = Double.longBitsToDouble(r.nextLong()); break;
        case 1: d = (r.nextDouble() - 0.5) * Math.pow(10, r.nextInt(30) - 12); break;
        case 2: d = Math.round(r.nextDouble() * 100000) / Math.pow(10, r.nextInt(8)); break;
        default: d = r.nextInt(2000000) * 0.25; break;
      }
      if (Double.isNaN(d)) continue;
      sb.append("d ").append(Long.toHexString(Double.doubleToRawLongBits(d))).append(' ').append(Double.toString(d)).append('\n');
      float f;
      switch (i % 3) {
        case 0: f = Float.intBitsToFloat(r.nextInt()); break;
        case 1: f = (float) ((r.nextDouble() - 0.5) * Math.pow(10, r.nextInt(20) - 8)); break;
        default: f = (float) (Math.round(r.nextDouble() * 10000) / Math.pow(10, r.nextInt(6))); break;
      }
      if (Float.isNaN(f)) continue;
      sb.append("f ").append(Integer.toHexString(Float.floatToRawIntBits(f))).append(' ').append(Float.toString(f)).append('\n');
    }
    System.out.print(sb);
  }
}
