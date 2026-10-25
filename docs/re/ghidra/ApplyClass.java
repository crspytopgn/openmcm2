import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.*;
import ghidra.program.model.data.*;
import ghidra.program.model.symbol.*;
import ghidra.program.model.address.*;
import java.util.*;
import java.nio.file.*;
// args: name vtableHex size funcListFile [extraVtables...]
public class ApplyClass extends GhidraScript {
  public void run() throws Exception {
    String[] a = getScriptArgs();
    String name = a[0]; Address vt = toAddr(Long.parseLong(a[1],16)); int size = Integer.parseInt(a[2],16);
    DataTypeManager dtm = currentProgram.getDataTypeManager();
    CategoryPath cp = new CategoryPath("/mm2"); CategoryPath root = CategoryPath.ROOT;
    StructureDataType vs = new StructureDataType(cp, name + "_vtbl", 0);
    List<Function> slots = new ArrayList<>();
    for (int i = 0; i < 200; i++) {
      Address e = vt.add(i*4);
      if (i > 0 && getSymbolAt(e) != null && getSymbolAt(e).getSource() != SourceType.DEFAULT && !getSymbolAt(e).getName().startsWith("PTR")) break;
      Function f = getFunctionAt(toAddr(getInt(e) & 0xffffffffL));
      if (f == null) break;
      slots.add(f);
      FunctionDefinitionDataType fd = new FunctionDefinitionDataType(f, false);
      try { fd.setNameAndCategory(cp, name + "_v" + i + "_" + f.getEntryPoint()); } catch (Exception ex) {}
      vs.add(new PointerDataType(fd), 4, "v" + i + "_" + f.getEntryPoint(), null);
    }
    DataType vsd = dtm.addDataType(vs, DataTypeConflictHandler.REPLACE_HANDLER);
    StructureDataType cs = new StructureDataType(root, name, 0);
    cs.add(new PointerDataType(vsd), 4, "vt", null);
    for (int off = 4; off < size; off += 4) cs.add(Undefined4DataType.dataType, 4, "f_" + Integer.toHexString(off), null);
    DataType csd = dtm.addDataType(cs, DataTypeConflictHandler.REPLACE_HANDLER);
    PointerDataType ptr = new PointerDataType(csd);
    Set<Function> targets = new LinkedHashSet<>();
    for (int k = 3; k < a.length; k++) {
      if (a[k].startsWith("vt:")) { Address v2 = toAddr(Long.parseLong(a[k].substring(3),16));
        for (int i=0;i<200;i++){ Function f=getFunctionAt(toAddr(getInt(v2.add(i*4))&0xffffffffL)); if(f==null)break; targets.add(f);} }
      else targets.add(getFunctionAt(toAddr(Long.parseLong(a[k],16))));
    }
    targets.addAll(slots);
    int n=0;
    ghidra.program.model.symbol.Namespace ns = currentProgram.getSymbolTable().getNamespace(name, currentProgram.getGlobalNamespace());
    if (ns == null) ns = currentProgram.getSymbolTable().createClass(currentProgram.getGlobalNamespace(), name, SourceType.USER_DEFINED);
    for (Function f : targets) {
      if (f == null) continue;
      try {
        f.setParentNamespace(ns);
        f.setCallingConvention("__thiscall");
        n++;
      } catch (Exception ex) { println("fail " + f.getEntryPoint() + " " + ex); }
    }
    println("typed " + n + " slots " + slots.size());
  }
}
