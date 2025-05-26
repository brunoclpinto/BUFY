//
//  String.extensions.swift
//  BUFY
//
//  Created by Bruno Pinto on 17/05/2025.
//

import BigInt // https://github.com/attaswift/BigInt


extension String {
  /// Validates if the string is a valid IBAN using basic structure and checksum validation.
  var isValidIBAN: Bool {
    // Remove spaces and make uppercase
    let cleaned = self.replacingOccurrences(of: " ", with: "").uppercased()
    
    // IBAN must be between 15 and 34 characters
    guard cleaned.count >= 15, cleaned.count <= 34 else { return false }
    
    // Move first 4 characters to the end
    let rearranged = cleaned.dropFirst(4) + cleaned.prefix(4)
    
    // Convert letters to numbers (A=10, B=11, ..., Z=35)
    let converted = rearranged.compactMap { char -> String? in
      if let digit = char.wholeNumberValue {
        return String(digit)
      } else if let ascii = char.asciiValue {
        return String(Int(ascii) - 55) // A=65 → 10
      }
      return nil
    }.joined()
    
    // Perform mod 97 check
    guard let bigIntValue = BigInt(converted) else { return false }
    return bigIntValue % 97 == 1
  }
}

extension String {
  /// Validates if the string is a valid SWIFT/BIC code.
  var isValidSWIFTOrBIC: Bool {
    let cleaned = self.uppercased()
    let regex = "^[A-Z]{4}[A-Z]{2}[A-Z0-9]{2}([A-Z0-9]{3})?$"
    return NSPredicate(format: "SELF MATCHES %@", regex).evaluate(with: cleaned)
  }
}

import Foundation

extension String {
  /// Returns true if the string contains only emoji characters (and optional whitespace).
  var containsOnlyEmoji: Bool {
    // Remove whitespace and variation selectors (used in emoji rendering)
    let cleaned = self.unicodeScalars.filter {
      !$0.properties.isWhitespace &&
      $0.value != 0xFE0F // Variation Selector-16
    }
    
    // Return false if empty after cleanup
    guard !cleaned.isEmpty else { return false }
    
    // Check each scalar is emoji
    for scalar in cleaned {
      if !scalar.properties.isEmoji {
        return false
      }
    }
    
    return true
  }
  
  var isValidCardNumber: Bool {
    let cleaned = self.replacingOccurrences(of: " ", with: "")
    guard cleaned.allSatisfy({ $0.isNumber }) else { return false }
    
    let digits = cleaned.reversed().compactMap { Int(String($0)) }
    
    var sum = 0
    for (index, digit) in digits.enumerated() {
      if index % 2 == 1 {
        let doubled = digit * 2
        sum += (doubled > 9) ? doubled - 9 : doubled
      } else {
        sum += digit
      }
    }
    
    return sum % 10 == 0
  }
}
