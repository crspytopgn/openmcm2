import ghidra.app.script.GhidraScript;
import ghidra.app.cmd.disassemble.DisassembleCommand;
import ghidra.app.cmd.function.CreateFunctionCmd;
import ghidra.program.model.address.*;
import java.io.*;
import java.nio.file.*;
public class MkFuncs extends GhidraScript {
  public void run() throws Exception {
    int n=0;
    for (String l : Files.readAllLines(Paths.get(getScriptArgs()[0]))) {
      l=l.trim(); if (l.isEmpty()) continue;
      Address a = toAddr(Long.parseLong(l,16));
      if (getFunctionAt(a) != null) continue;
      if (getFunctionContaining(a) != null) { println("inside " + l); }
      new DisassembleCommand(a, null, true).applyTo(currentProgram, monitor);
      if (new CreateFunctionCmd(a).applyTo(currentProgram, monitor)) n++;
    }
    println("created " + n);
  }
}
