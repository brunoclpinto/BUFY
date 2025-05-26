//
//  File.swift
//  BUFY
//
//  Created by Bruno Pinto on 16/11/2024.
//

import Foundation

protocol EntityProtocol: Hashable, Codable {
  var name: String { get }
  var description: String { get set }
  var category: Category { get set }
}

extension EntityProtocol {
  func hash(into hasher: inout Hasher) {
    hasher.combine(name.lowercased())
  }
  
  static func == (lhs: Self, rhs: Self) -> Bool {
    lhs.name.lowercased() == rhs.name.lowercased()
  }
  
  static func isValidName(_ name: String) throws {
    guard !name.isEmpty else {
      throw CustomError.invalidArgument(
        argument: "Name",
        currentValue: name,
        expected: "Not empty",
        classType: Self.self
      )
    }
  }
  
  static func isValidIBAN(_ iban: String) throws {
    guard !iban.isEmpty else {
      throw CustomError.invalidArgument(
        argument: "IBAN",
        currentValue: iban,
        expected: "not empty",
        classType: Self.self
      )
    }
    guard iban.isValidIBAN else {
      throw CustomError.invalidArgument(
        argument: "IBAN",
        currentValue: iban,
        expected: "valid format",
        classType: Self.self
      )
    }
  }
  
  static func isValidSwiftBIC(_ swiftBIC: String) throws {
    guard !swiftBIC.isEmpty else {
      return
    }
    guard swiftBIC.isValidSWIFTOrBIC else {
      throw CustomError.invalidArgument(
        argument: "swiftBIC",
        currentValue: swiftBIC,
        expected: "valid format",
        classType: Self.self
      )
    }
  }
  
  static func isValidTAEG(_ taeg: Float) throws {
    guard taeg >= 0 else {
      throw CustomError.invalidArgument(
        argument: "TAEG",
        currentValue: "\(taeg)",
        expected: ">= 0",
        classType: Self.self
      )
    }
  }
  
  static func isValidCardNumber(_ cardNumber: String ) throws {
    guard !cardNumber.isEmpty else {
      return
    }
    
    guard cardNumber.isValidCardNumber else {
      throw CustomError.invalidArgument(
        argument: "cardNumber",
        currentValue: cardNumber,
        expected: "Valid credit or debit card number",
        classType: Credit.self
      )
    }
  }
}

struct Entity: EntityProtocol {
  private(set) var name: String
  var description: String = ""
  var category: Category
  
  init(
    name: String,
    description: String = "",
    category: Category
  ) throws {
    try Self.isValidName(name)
    
    self.name = name
    self.description = description
    self.category = category
  }
  
  mutating func setName(_ name: String) throws {
    try Self.isValidName(name)
    self.name = name
  }
}

struct Account: EntityProtocol {
  private(set) var name: String
  var description: String = ""
  var category: Category
  private(set) var iban: String
  private(set) var swiftBIC: String
  private(set) var taeg: Float = 0
  var isSavings: Bool = false
  
  init(
    name: String,
    description: String = "",
    category: Category,
    iban: String,
    swiftBIC: String = "",
    taeg: Float = 0,
    isSavings: Bool
  ) throws {
    try Self.isValidName(name)
    try Self.isValidIBAN(iban)
    try Self.isValidSwiftBIC(swiftBIC)
    try Self.isValidTAEG(taeg)
    
    self.name = name
    self.description = description
    self.category = category
    self.iban = iban
    self.swiftBIC = swiftBIC
    self.isSavings = isSavings
  }
  
  mutating func setName(_ name: String) throws {
    try Self.isValidName(name)
    self.name = name
  }
  
  mutating func setIBAN(_ iban: String) throws {
    try Self.isValidIBAN(iban)
    self.iban = iban
  }
  
  mutating func setSwiftBIC(_ swiftBIC: String) throws {
    try Self.isValidSwiftBIC(swiftBIC)
    self.swiftBIC = swiftBIC
  }
  
  mutating func setTAEG(_ taeg: Float) throws {
    try Self.isValidTAEG(taeg)
    self.taeg = taeg
  }
}

struct Credit: EntityProtocol {
  private(set) var name: String
  var description: String = ""
  var category: Category
  private(set) var cardNumber: String
  private(set) var taeg: Float = 0
  
  init(
    name: String,
    description: String = "",
    category: Category,
    cardNumber: String = "",
    taeg: Float = 0
  ) throws {
    try Self.isValidName(name)
    try Self.isValidCardNumber(cardNumber)
    try Self.isValidTAEG(taeg)
    
    self.name = name
    self.description = description
    self.category = category
    self.cardNumber = cardNumber
    self.taeg = taeg
  }
  
  mutating func setName(_ name: String) throws {
    try Self.isValidName(name)
    self.name = name
  }
  
  mutating func setCardNumber(_ cardNumber: String) throws {
    try Self.isValidCardNumber(cardNumber)
    self.cardNumber = cardNumber
  }
}
