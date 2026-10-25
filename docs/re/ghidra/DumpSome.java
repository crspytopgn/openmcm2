import ghidra.app.script.GhidraScript;
import ghidra.app.decompiler.*;
import ghidra.program.model.listing.*;
import java.io.*;
import java.nio.file.*;
public class DumpSome extends GhidraScript {
  public void run() throws Exception {
    String[] a = getScriptArgs();
    DecompInterface d = new DecompInterface(); d.openProgram(currentProgram);
    for (String l : Files.readAllLines(Paths.get(a[0]))) {
      l=l.trim(); if (l.isEmpty()) continue;
      Function f = getFunctionAt(toAddr(Long.parseLong(l,16))); if (f==null) continue;
      DecompileResults r = d.decompileFunction(f, 90, monitor);
      PrintWriter pw = new PrintWriter(new FileWriter(a[1] + "/" + String.format("%08x", Long.parseLong(l,16)) + ".c"));
      pw.println("//==== " + f.getEntryPoint() + " " + f.getName(true));
      pw.println(r.decompileCompleted() ? r.getDecompiledFunction().getC() : "// failed");
      pw.close();
    }
  }
}
