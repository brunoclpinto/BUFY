//
//  Errors.swift
//  BUFY
//
//  Created by Bruno Pinto on 22/11/2024.
//

/// Base struct for error creation.
/// All erros ere expected to comply with constraints and definitions in this struct.
///
/// Please extend to add new errors
struct CustomError: Error, Identifiable {
  /// Must have a unique number.
  let id: CustomErrorID
  /// Human readable information to clarify error reason.
  let message: String
  /// Which file path triggered.
  let filePath: String
  /// Which class triggered.
  let className: String
  /// Which function triggered.
  let functionName: String
  /// Which line number triggered.
  let lineNumber: Int
  
  var description: String {
    return "Error in \(filePath).\(className).\(functionName) at line \(lineNumber): \(message)"
  }
  
  init?(
    id: CustomErrorID,
    message: String,
    filePath: String,
    className: String,
    functionName: String,
    lineNumber: Int
  ) {
    assertionFailure("Forbiden init method for \(type(of: self))")
    return nil
  }
  
  init<T>(
    id: CustomErrorID,
    message: String,
    filePath: String = #file,
    classType: T.Type = T.self,
    functionName: String = #function,
    lineNumber: Int = #line
  ) {
    self.id = id
    self.message = message
    self.filePath = filePath
    self.className = "\(classType)"
    self.functionName = functionName
    self.lineNumber = lineNumber
  }
}

/// Each defined error must have an unique ID.
enum CustomErrorID: Int {
  case invalidArgument = 1
}

/// Each Error is expected to have a static func that handles its creation according to error description needs.
extension CustomError {
  static func invalidArgument<T>(
    argument: String,
    currentValue: String,
    expected: String,
    filePath: String = #file,
    classType: T.Type,
    functionName: String = #function,
    lineNumber: Int = #line
  ) -> CustomError {
    CustomError(
      id: .invalidArgument,
      message: "Invalid argument: \(argument) with value \(currentValue) | Expected: \(expected)",
      filePath: filePath,
      classType: classType,
      functionName: functionName,
      lineNumber: lineNumber
    )
  }
}
