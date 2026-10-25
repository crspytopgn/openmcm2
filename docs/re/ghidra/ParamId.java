import ghidra.app.script.GhidraScript;
import ghidra.app.plugin.core.analysis.AutoAnalysisManager;
import ghidra.program.model.address.*;
public class ParamId extends GhidraScript {
  public void run() throws Exception {
    setAnalysisOption(currentProgram, "Decompiler Parameter ID", "true");
    AutoAnalysisManager m = AutoAnalysisManager.getAnalysisManager(currentProgram);
    m.reAnalyzeAll(currentProgram.getMemory().getLoadedAndInitializedAddressSet());
    m.startAnalysis(monitor);
    println("reanalyzed");
  }
}
