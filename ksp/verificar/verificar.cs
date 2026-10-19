// Compila arquivos .ks com o compilador do kOS (kOS.Safe.dll), por reflexão:
// o compilador é interno. Sai com erro se algum não compila.
using System; using System.IO; using System.Linq; using System.Reflection; using kOS.Safe.Compilation.KS;
class P { static int Main(string[] args) {
  var asm = typeof(Parser).Assembly;
  var tComp = asm.GetType("kOS.Safe.Compilation.KS.Compiler");
  var tCtx = asm.GetType("kOS.Safe.Compilation.KS.Context");
  var tOpt = asm.GetType("kOS.Safe.Compilation.CompilerOptions");
  var F = BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Instance;
  int bad = 0;
  foreach (var f in args) {
    try {
      var tree = new Parser(new Scanner()).Parse(File.ReadAllText(f));
      if (tree.Errors.Count > 0) { bad++; Console.WriteLine(f + ": sintaxe, linha " + tree.Errors[0].Line + ": " + tree.Errors[0].Message); continue; }
      var comp = Activator.CreateInstance(tComp, true);
      var ctx = Activator.CreateInstance(tCtx, true);
      var opts = Activator.CreateInstance(tOpt, true);
      var m = tComp.GetMethods(F).First(x => x.Name == "Compile");
      var ps = m.GetParameters();
      var a = new object[ps.Length];
      for (int i = 0; i < ps.Length; i++) {
        var pt = ps[i].ParameterType;
        if (pt == typeof(int)) a[i] = 1;
        else if (pt == typeof(ParseTree)) a[i] = tree;
        else if (pt == tCtx) a[i] = ctx;
        else if (pt == tOpt) a[i] = opts;
        else a[i] = null;
      }
      m.Invoke(comp, a);
      Console.WriteLine("compila   " + f);
    } catch (Exception e) { bad++; var ie = e.InnerException ?? e; Console.WriteLine(f + ": " + ie.GetType().Name + ": " + ie.Message); }
  }
  return bad;
}}
