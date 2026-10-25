import ghidra.app.script.GhidraScript;
import ghidra.app.decompiler.*;
import ghidra.program.model.listing.*;
import ghidra.program.model.symbol.*;
import ghidra.program.model.data.*;
import ghidra.program.model.address.*;
import java.io.*;
import ghidra.program.util.DefinedDataIterator;
public class DumpAll extends GhidraScript {
  public void run() throws Exception {
    String out = getScriptArgs()[0];
    DecompInterface d = new DecompInterface();
    d.openProgram(currentProgram);
    PrintWriter pw = new PrintWriter(new FileWriter(out + "/decomp.c"));
    PrintWriter fl = new PrintWriter(new FileWriter(out + "/functions.tsv"));
    FunctionIterator it = currentProgram.getFunctionManager().getFunctions(true);
    while (it.hasNext() && !monitor.isCancelled()) {
      Function f = it.next();
      fl.println(f.getEntryPoint() + "\t" + f.getName() + "\t" + f.getBody().getNumAddresses());
      DecompileResults r = d.decompileFunction(f, 60, monitor);
      pw.println("//==== " + f.getEntryPoint() + " " + f.getName());
      if (r != null && r.decompileCompleted()) pw.println(r.getDecompiledFunction().getC());
      else pw.println("// decompile failed");
    }
    pw.close(); fl.close();
    PrintWriter sw = new PrintWriter(new FileWriter(out + "/strings.tsv"));
    DataIterator di = currentProgram.getListing().getDefinedData(true);
    while (di.hasNext()) { Data s = di.next(); if (!s.hasStringValue()) continue;
      StringBuilder refs = new StringBuilder();
      for (Reference ref : getReferencesTo(s.getAddress())) {
        Function f = getFunctionContaining(ref.getFromAddress());
        refs.append(ref.getFromAddress()).append("(").append(f==null?"?":f.getName()).append(") ");
      }
      Object v = s.getValue();
      sw.println(s.getAddress() + "\t" + String.valueOf(v).replace("\n","\\n").replace("\t","\\t") + "\t" + refs);
    }
    sw.close();
  }
}
