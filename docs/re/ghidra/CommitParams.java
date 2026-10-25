import ghidra.app.script.GhidraScript;
import ghidra.app.decompiler.*;
import ghidra.program.model.listing.*;
import ghidra.program.model.pcode.*;
import ghidra.program.model.symbol.SourceType;
public class CommitParams extends GhidraScript {
  public void run() throws Exception {
    DecompInterface d = new DecompInterface(); d.openProgram(currentProgram);
    int n=0, ok=0;
    for (Function f : currentProgram.getFunctionManager().getFunctions(true)) {
      if (f.getSignatureSource() == SourceType.USER_DEFINED || f.isThunk()) continue;
      DecompileResults r = d.decompileFunction(f, 60, monitor);
      n++;
      if (r == null || !r.decompileCompleted()) continue;
      HighFunction h = r.getHighFunction();
      try {
        HighFunctionDBUtil.commitParamsToDatabase(h, true, HighFunctionDBUtil.ReturnCommitOption.COMMIT, SourceType.ANALYSIS);
        ok++;
      } catch (Exception e) {}
    }
    println("commit " + ok + "/" + n);
  }
}
