import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.*;
import ghidra.program.model.symbol.*;
import ghidra.program.model.address.*;
import java.io.*;
public class DumpMeta extends GhidraScript {
  public void run() throws Exception {
    String out = getScriptArgs()[0];
    PrintWriter sy = new PrintWriter(new FileWriter(out + "/symbols.tsv"));
    SymbolIterator si = currentProgram.getSymbolTable().getAllSymbols(true);
    while (si.hasNext()) { Symbol s = si.next();
      if (s.getSource() == SourceType.DEFAULT) continue;
      sy.println(s.getAddress() + "\t" + s.getName(true) + "\t" + s.getSymbolType());
      if (s.getName().equals("vftable")) {
        Address a = s.getAddress();
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < 80; i++) {
          Address e = a.add(i*4);
          if (i > 0 && currentProgram.getSymbolTable().getPrimarySymbol(e) != null && currentProgram.getSymbolTable().getPrimarySymbol(e).getSource()!=SourceType.DEFAULT) break;
          int v = getInt(e);
          Address t = toAddr(v & 0xffffffffL);
          Function f = getFunctionAt(t);
          if (f == null) break;
          sb.append(i).append("=").append(f.getEntryPoint()).append(" ");
        }
        sy.println("VT\t" + s.getName(true) + "\t" + sb);
      }
    }
    sy.close();
    PrintWriter cg = new PrintWriter(new FileWriter(out + "/callgraph.tsv"));
    for (Function f : currentProgram.getFunctionManager().getFunctions(true)) {
      StringBuilder sb = new StringBuilder();
      for (Function c : f.getCalledFunctions(monitor)) sb.append(c.getEntryPoint()).append(" ");
      cg.println(f.getEntryPoint() + "\t" + sb);
    }
    cg.close();
  }
}
